use crate::{
  HookResolveIdArgs, PluginDriver,
  types::{custom_field::CustomField, hook_resolve_id_skipped::HookResolveIdSkipped},
};
use rolldown_common::{
  ImportKind, ModuleDefFormat, PackageJson, ResolvedId, is_existing_node_builtin_modules,
};
use rolldown_resolver::{ResolveError, Resolver};
use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

fn is_http_url(s: &str) -> bool {
  s.starts_with("http://") || s.starts_with("https://") || s.starts_with("//")
}

pub fn is_data_url(s: &str) -> bool {
  s.trim_start().starts_with("data:")
}

/// Infers ModuleDefFormat from file path and optional package.json.
/// This matches the logic used in the internal resolver's `infer_module_def_format`.
fn infer_module_def_format(path: &str, package_json: Option<&Arc<PackageJson>>) -> ModuleDefFormat {
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

#[expect(clippy::too_many_arguments)]
pub async fn resolve_id_with_plugins(
  resolver: &Resolver,
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
      let package_json = r.package_json_path.as_ref().and_then(|p| {
        let v = resolver.package_json_cache().get(&PathBuf::from(&p))?;
        let package_json = v.clone();
        Some(package_json)
      });
      return Ok(Ok(ResolvedId {
        module_def_format: infer_module_def_format(r.id.as_str(), package_json.as_ref()),
        id: r.id,
        external: r.external.unwrap_or_default(),
        normalize_external_id: r.normalize_external_id,
        side_effects: r.side_effects,
        package_json,
        ..Default::default()
      }));
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
    let package_json = r.package_json_path.as_ref().and_then(|p| {
      let v = resolver.package_json_cache().get(&PathBuf::from(&p))?;
      let package_json = v.clone();
      Some(package_json)
    });
    return Ok(Ok(ResolvedId {
      module_def_format: infer_module_def_format(r.id.as_str(), package_json.as_ref()),
      id: r.id,
      external: r.external.unwrap_or_default(),
      normalize_external_id: r.normalize_external_id,
      side_effects: r.side_effects,
      package_json,
      ..Default::default()
    }));
  }

  // Auto external http url or data url
  if is_http_url(specifier) || is_data_url(specifier) {
    return Ok(Ok(ResolvedId {
      id: specifier.into(),
      external: true.into(),
      ..Default::default()
    }));
  }

  Ok(resolve_id(resolver, specifier, importer, import_kind, is_user_defined_entry))
}

fn resolve_id(
  resolver: &Resolver,
  specifier: &str,
  importer: Option<&str>,
  import_kind: ImportKind,
  is_user_defined_entry: bool,
) -> Result<ResolvedId, ResolveError> {
  let resolved =
    resolver.resolve(importer.map(Path::new), specifier, import_kind, is_user_defined_entry);

  match resolved {
    Ok(resolved) => Ok(ResolvedId::from(resolved)),
    Err(err) => match err {
      ResolveError::Builtin { resolved, is_runtime_module } => Ok(ResolvedId {
        // `resolved` is always prefixed with "node:" in compliance with the ESM specification.
        // we needs to use `is_runtime_module` to get the original specifier
        is_external_without_side_effects: is_existing_node_builtin_modules(&resolved),
        id: if resolved.starts_with("node:") && !is_runtime_module {
          resolved[5..].into()
        } else {
          resolved.into()
        },
        external: true.into(),
        ..Default::default()
      }),
      ResolveError::Ignored(p) => Ok(ResolvedId {
        //(hyf0) TODO: This `p` doesn't seem to contains `query` or `fragment` of the input. We need to make sure this is ok
        id: p.to_str().expect("Should be valid utf8").into(),
        ignored: true,
        ..Default::default()
      }),
      _ => Err(err),
    },
  }
}
