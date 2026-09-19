use crate::{
  HookResolveIdArgs, HookResolveIdOutput, PluginDriver,
  types::{custom_field::CustomField, hook_resolve_id_skipped::HookResolveIdSkipped},
};
use nodejs_built_in_modules::is_nodejs_builtin_module;
use rolldown_common::{ImportKind, ModuleDefFormat, ModuleId, PackageJson, ResolvedId};
use rolldown_fs::FileSystem;
use rolldown_resolver::{PackageScopes, ResolveError, Resolver};
use rolldown_utils::dataurl::is_data_url;
use std::{path::Path, sync::Arc};
use sugar_path::SugarPath;

fn is_http_url(s: &str) -> bool {
  s.starts_with("http://") || s.starts_with("https://") || s.starts_with("//")
}

/// Infers ModuleDefFormat from file path and optional package.json.
/// This matches the logic used in the internal resolver's `infer_module_def_format`.
pub fn infer_module_def_format(
  path: &str,
  package_json: Option<&Arc<PackageJson>>,
) -> ModuleDefFormat {
  let fmt = ModuleDefFormat::from_path(path);

  // If the extension is specific (.mjs/.cjs/.cts/.mts), use it
  if !matches!(fmt, ModuleDefFormat::Unknown) {
    return fmt;
  }

  // Check if it's a js-like extension (.js/.jsx/.ts/.tsx)
  let is_js_like_extension = Path::new(path)
    .extension()
    .is_some_and(|ext| matches!(ext.to_str(), Some("js" | "jsx" | "ts" | "tsx")));

  if is_js_like_extension {
    if let Some(pkg) = package_json {
      if let Some(type_field) = pkg.r#type() {
        return match type_field {
          "module" => ModuleDefFormat::EsmPackageJson,
          "commonjs" => ModuleDefFormat::CjsPackageJson,
          _ => ModuleDefFormat::Unknown,
        };
      }
    }
  }

  ModuleDefFormat::Unknown
}

/// Applies normal resolution's precedence for `"type"` to the recovered manifests.
///
/// `oxc_resolver` reads `"type"` from the nearest scope for `.js` and `.ts` only, and rolldown
/// then falls back to the owning manifest for every js-like extension. Taking the nearest
/// manifest for all four would invert the answer for `.jsx` and `.tsx`.
fn recovered_module_def_format(id: &str, scopes: &PackageScopes) -> ModuleDefFormat {
  let oxc_reads_nearest =
    matches!(Path::new(id).extension().and_then(|ext| ext.to_str()), Some("js" | "ts"));
  if oxc_reads_nearest {
    let format = infer_module_def_format(id, scopes.nearest.as_ref());
    if !matches!(format, ModuleDefFormat::Unknown) {
      return format;
    }
  }
  infer_module_def_format(id, scopes.side_effects_owner.as_ref())
}

/// Builds the `ResolvedId` for an id a plugin resolved.
///
/// A `resolveId` hook may return a bare id string, which carries no `packageJsonPath`, so the
/// manifests are recovered from the id instead. Otherwise a module's `package.json#sideEffects`
/// policy and its module format would depend on the specifier that reached it.
/// See `internal-docs/module-side-effects/design.md` and
/// <https://github.com/rolldown/rolldown/issues/10909>.
fn resolved_id_from_hook_output<Fs: FileSystem>(
  resolver: &Resolver<Fs>,
  r: HookResolveIdOutput,
) -> anyhow::Result<ResolvedId> {
  let id = ModuleId::new(r.id);
  // The hook's own `packageJsonPath` is its answer for both questions. Only the recovery path
  // has to tell them apart, because `"type"` and `sideEffects` come from different manifests.
  let scopes = match &r.package_json_path {
    Some(path) => {
      let package_json = resolver.try_get_package_json_or_create(path.as_path())?;
      PackageScopes {
        nearest: Some(Arc::clone(&package_json)),
        side_effects_owner: Some(package_json),
      }
    }
    // Only a real filesystem id has a package to find; virtual and bare ids have none.
    None => match id.as_path() {
      Some(path) => resolver.package_scopes(path)?,
      None => PackageScopes::default(),
    },
  };
  Ok(ResolvedId {
    module_def_format: recovered_module_def_format(id.as_str(), &scopes),
    id,
    external: r.external.unwrap_or_default(),
    normalize_external_id: r.normalize_external_id,
    side_effects: r.side_effects,
    package_json: scopes.side_effects_owner,
    ..Default::default()
  })
}

#[expect(clippy::too_many_arguments)]
pub async fn resolve_id_with_plugins<Fs: FileSystem>(
  resolver: &Resolver<Fs>,
  plugin_driver: &PluginDriver,
  specifier: &str,
  importer: Option<&str>,
  is_entry: bool,
  import_kind: ImportKind,
  skipped_resolve_calls: Option<Vec<Arc<HookResolveIdSkipped>>>,
  custom: Arc<CustomField>,
  is_user_defined_entry: bool,
) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
  if matches!(import_kind, ImportKind::DynamicImport) {
    if let Some(r) = plugin_driver
      .resolve_dynamic_import(
        &HookResolveIdArgs {
          importer: importer.map(std::convert::AsRef::as_ref),
          specifier,
          is_entry,
          kind: import_kind,
          custom: Arc::clone(&custom),
        },
        skipped_resolve_calls.as_ref(),
      )
      .await?
    {
      return Ok(Ok(resolved_id_from_hook_output(resolver, r)?));
    }
  }
  // Run plugin resolve_id first, if it is None use internal resolver as fallback
  if let Some(r) = plugin_driver
    .resolve_id(
      &HookResolveIdArgs {
        specifier,
        importer,
        is_entry,
        kind: import_kind,
        custom: Arc::clone(&custom),
      },
      skipped_resolve_calls.as_ref(),
    )
    .await?
  {
    return Ok(Ok(resolved_id_from_hook_output(resolver, r)?));
  }

  // Auto external http url or data url
  if is_http_url(specifier) || is_data_url(specifier) {
    return Ok(Ok(ResolvedId {
      id: ModuleId::new(specifier),
      external: true.into(),
      ..Default::default()
    }));
  }

  Ok(resolve_id(resolver, specifier, importer, import_kind, is_user_defined_entry))
}

fn resolve_id<Fs: FileSystem>(
  resolver: &Resolver<Fs>,
  specifier: &str,
  importer: Option<&str>,
  import_kind: ImportKind,
  is_user_defined_entry: bool,
) -> Result<ResolvedId, ResolveError> {
  // Data URL modules have no filesystem location, so imports from them cannot be resolved.
  if importer.is_some_and(|id| id.starts_with("\0rolldown/data-url:")) {
    return Err(ResolveError::NotFound(specifier.to_string()));
  }
  let resolved =
    resolver.resolve(importer.map(Path::new), specifier, import_kind, is_user_defined_entry);

  match resolved {
    Ok(resolved) => Ok(ResolvedId::from(resolved)),
    Err(err) => match err {
      ResolveError::Builtin { resolved, is_runtime_module } => Ok(ResolvedId {
        // `resolved` is always prefixed with "node:" in compliance with the ESM specification.
        // we needs to use `is_runtime_module` to get the original specifier
        is_external_without_side_effects: is_nodejs_builtin_module(&resolved),
        id: ModuleId::new(if resolved.starts_with("node:") && !is_runtime_module {
          &resolved[5..]
        } else {
          &resolved
        }),
        external: true.into(),
        ..Default::default()
      }),
      ResolveError::Ignored(p) => Ok(ResolvedId {
        id: ModuleId::new_empty(p.to_str().expect("Should be valid utf8")),
        ..Default::default()
      }),
      _ => Err(err),
    },
  }
}
