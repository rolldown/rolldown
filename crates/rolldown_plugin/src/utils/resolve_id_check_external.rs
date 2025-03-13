use crate::{
  PluginDriver,
  types::{custom_field::CustomField, hook_resolve_id_skipped::HookResolveIdSkipped},
};
use arcstr::ArcStr;
use rolldown_common::{
  ImportKind, MakeAbsoluteExternalsRelative, ModuleDefFormat, ResolvedExternal, ResolvedId,
  SharedNormalizedBundlerOptions,
};
use rolldown_resolver::{ResolveError, Resolver};
use std::{path::Path, sync::Arc};
use sugar_path::SugarPath;

use crate::__inner::resolve_id_with_plugins;

use super::resolve_id_with_plugins::is_data_url;

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
  if let Some(resolve_id) =
    check_external_with_request(request, importer, bundle_options, false).await?
  {
    return Ok(Ok(resolve_id));
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
      if matches!(resolved_id.external, ResolvedExternal::Bool(false)) {
        // Check external with resolved path
        if let Some(is_external) = bundle_options.external.as_ref() {
          resolved_id.external = is_external(resolved_id.id.as_str(), importer, true).await?.into();
        }
      }

      match resolved_id.external {
        ResolvedExternal::Bool(true) => {
          if !is_absolute(resolved_id.id.as_str())
            || is_not_absolute_external(
              resolved_id.id.as_str(),
              request,
              &bundle_options.make_absolute_externals_relative,
            )
          {
            resolved_id.external = true.into();
          } else {
            resolved_id.external = ResolvedExternal::Absolute;
          }

          if matches!(resolved_id.normalize_external_id, Some(true))
            && bundle_options.make_absolute_externals_relative.is_enabled()
          {
            resolved_id.id =
              normalize_relative_external_id(&resolved_id.id, importer, &bundle_options.cwd);
          }
        }
        ResolvedExternal::Absolute => {
          if is_absolute(resolved_id.id.as_str()) {
            resolved_id.external = ResolvedExternal::Absolute;
          } else {
            resolved_id.external = true.into();
          }
        }
        ResolvedExternal::Relative => {
          resolved_id.external = true.into();
        }
        ResolvedExternal::Bool(false) => {}
      }

      Ok(Ok(resolved_id))
    }
    Err(e) => {
      if let ResolveError::NotFound(_) = &e {
        // If module can't resolve, check external with unresolved path with `isResolved: true`
        if let Some(resolve_id) =
          check_external_with_request(request, importer, bundle_options, true).await?
        {
          return Ok(Ok(resolve_id));
        }
      }
      Ok(Err(e))
    }
  }
}

async fn check_external_with_request(
  request: &str,
  importer: Option<&str>,
  bundle_options: &SharedNormalizedBundlerOptions,
  is_resolved: bool,
) -> anyhow::Result<Option<ResolvedId>> {
  // ref https://github.com/rollup/rollup/blob/master/src/ModuleLoader.ts#L555
  if let Some(is_external) = bundle_options.external.as_ref() {
    let id = if bundle_options.make_absolute_externals_relative.is_enabled() {
      normalize_relative_external_id(request, importer, &bundle_options.cwd)
    } else {
      request.into()
    };
    let raw_request = if is_resolved { &id } else { request };
    if is_external(raw_request, importer, is_resolved).await? {
      let external =
        if is_not_absolute_external(&id, request, &bundle_options.make_absolute_externals_relative)
        {
          true.into()
        } else {
          ResolvedExternal::Absolute
        };
      return Ok(Some(ResolvedId {
        id,
        ignored: false,
        module_def_format: ModuleDefFormat::Unknown,
        external,
        normalize_external_id: None,
        package_json: None,
        side_effects: None,
        is_external_without_side_effects: false,
      }));
    }
  }
  Ok(None)
}

fn is_not_absolute_external(
  id: &str,
  source: &str,
  make_absolute_externals_relative: &MakeAbsoluteExternalsRelative,
) -> bool {
  matches!(make_absolute_externals_relative, MakeAbsoluteExternalsRelative::Bool(true))
    || (matches!(make_absolute_externals_relative, MakeAbsoluteExternalsRelative::IfRelativeSource)
      && is_relative(source))
    || !is_absolute(id)
}

fn normalize_relative_external_id(source: &str, importer: Option<&str>, root: &Path) -> ArcStr {
  if is_relative(source) {
    if let Some(importer) = importer {
      if is_data_url(importer) {
        source.into()
      } else {
        Path::new(importer).join("..").join(source).normalize().to_string_lossy().into()
      }
    } else {
      root.join(source).normalize().to_string_lossy().into()
    }
  } else {
    source.into()
  }
}

#[inline]
fn is_absolute(id: &str) -> bool {
  Path::new(id).is_absolute() || id.starts_with('/')
}

#[inline]
fn is_relative(id: &str) -> bool {
  id.starts_with("./") || id.starts_with("../")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_relative() {
    assert!(!is_relative("path"));
    assert!(is_relative("./a.js"));
    assert!(is_relative("../a.js"));
  }

  #[test]
  fn test_is_absolute() {
    assert!(is_absolute("/a.js")); // make sure it is true at windows
  }
}
