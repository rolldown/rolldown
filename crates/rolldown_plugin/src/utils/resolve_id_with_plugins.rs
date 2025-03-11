use crate::{
  HookResolveIdArgs, PluginDriver,
  types::{custom_field::CustomField, hook_resolve_id_skipped::HookResolveIdSkipped},
};
use rolldown_common::{
  ImportKind, ModuleDefFormat, ResolvedId, SharedNormalizedBundlerOptions,
  is_existing_node_builtin_modules,
};
use rolldown_resolver::{ResolveError, Resolver};
use std::{path::Path, sync::Arc};

fn is_http_url(s: &str) -> bool {
  s.starts_with("http://") || s.starts_with("https://") || s.starts_with("//")
}

fn is_data_url(s: &str) -> bool {
  s.trim_start().starts_with("data:")
}

#[allow(clippy::too_many_arguments)]
pub async fn resolve_id_check_external(
  resolver: &Resolver,
  plugin_driver: &PluginDriver,
  request: &str,
  importer: Option<&str>,
  is_entry: bool,
  import_kind: ImportKind,
  skipped_resolve_calls: Option<Vec<Arc<HookResolveIdSkipped>>>,
  custom: Arc<CustomField>,
  is_user_defined_entry: bool,
  bundle_options: &SharedNormalizedBundlerOptions,
) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
  // Check external with unresolved path
  if let Some(is_external) = bundle_options.external.as_ref() {
    if is_external(request, importer, false).await? {
      return Ok(Ok(ResolvedId {
        id: request.into(),
        ignored: false,
        module_def_format: ModuleDefFormat::Unknown,
        external: true.into(),
        package_json: None,
        side_effects: None,
        is_external_without_side_effects: false,
      }));
    }
  }

  let resolved_id = resolve_id_with_plugins(
    resolver,
    plugin_driver,
    request,
    importer,
    is_entry,
    import_kind,
    skipped_resolve_calls,
    custom,
    is_user_defined_entry,
  )
  .await?;

  match resolved_id {
    Ok(mut resolved_id) => {
      if !resolved_id.external.is_external() {
        // Check external with resolved path
        if let Some(is_external) = bundle_options.external.as_ref() {
          resolved_id.external = is_external(resolved_id.id.as_str(), importer, true).await?.into();
        }
      }
      Ok(Ok(resolved_id))
    }
    Err(e) => {
      if let ResolveError::NotFound(_) = &e {
        // If module can't resolve, check external with unresolved path with `isResolved: true`
        // ref https://github.com/rollup/rollup/blob/master/src/ModuleLoader.ts#L555
        if let Some(is_external) = bundle_options.external.as_ref() {
          if is_external(request, importer, true).await? {
            return Ok(Ok(ResolvedId {
              id: request.into(),
              ignored: false,
              module_def_format: ModuleDefFormat::Unknown,
              external: true.into(),
              package_json: None,
              side_effects: None,
              is_external_without_side_effects: false,
            }));
          }
        }
      }
      Ok(Err(e))
    }
  }
}

#[allow(clippy::too_many_arguments)]
pub async fn resolve_id_with_plugins(
  resolver: &Resolver,
  plugin_driver: &PluginDriver,
  request: &str,
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
          specifier: request,
          is_entry,
          kind: import_kind,
          custom: Arc::clone(&custom),
        },
        skipped_resolve_calls.as_ref(),
      )
      .await?
    {
      return Ok(Ok(ResolvedId {
        module_def_format: ModuleDefFormat::from_path(r.id.as_str()),
        ignored: false,
        id: r.id,
        external: r.external.unwrap_or_default(),
        package_json: None,
        side_effects: r.side_effects,
        is_external_without_side_effects: false,
      }));
    }
  }
  // Run plugin resolve_id first, if it is None use internal resolver as fallback
  if let Some(r) = plugin_driver
    .resolve_id(
      &HookResolveIdArgs {
        importer: importer.map(std::convert::AsRef::as_ref),
        specifier: request,
        is_entry,
        kind: import_kind,
        custom: Arc::clone(&custom),
      },
      skipped_resolve_calls.as_ref(),
    )
    .await?
  {
    return Ok(Ok(ResolvedId {
      module_def_format: ModuleDefFormat::from_path(r.id.as_str()),
      ignored: false,
      id: r.id,
      external: r.external.unwrap_or_default(),
      package_json: None,
      side_effects: r.side_effects,
      is_external_without_side_effects: false,
    }));
  }

  // Auto external http url or data url
  if is_http_url(request) || is_data_url(request) {
    return Ok(Ok(ResolvedId {
      id: request.into(),
      module_def_format: ModuleDefFormat::Unknown,
      ignored: false,
      external: true.into(),
      package_json: None,
      side_effects: None,
      is_external_without_side_effects: false,
    }));
  }

  Ok(resolve_id(resolver, request, importer, import_kind, is_user_defined_entry))
}

fn resolve_id(
  resolver: &Resolver,
  request: &str,
  importer: Option<&str>,
  import_kind: ImportKind,
  is_user_defined_entry: bool,
) -> Result<ResolvedId, ResolveError> {
  let resolved =
    resolver.resolve(importer.map(Path::new), request, import_kind, is_user_defined_entry);

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
        ignored: false,
        external: true.into(),
        module_def_format: ModuleDefFormat::Unknown,
        package_json: None,
        side_effects: None,
      }),
      ResolveError::Ignored(p) => Ok(ResolvedId {
        //(hyf0) TODO: This `p` doesn't seem to contains `query` or `fragment` of the input. We need to make sure this is ok
        id: p.to_str().expect("Should be valid utf8").into(),
        ignored: true,
        external: false.into(),
        module_def_format: ModuleDefFormat::Unknown,
        package_json: None,
        side_effects: None,
        is_external_without_side_effects: false,
      }),
      _ => Err(err),
    },
  }
}
