use crate::{
  PluginDriver,
  types::{custom_field::CustomField, hook_resolve_id_skipped::HookResolveIdSkipped},
};
use arcstr::ArcStr;
use rolldown_common::{
  ImportKind, MakeAbsoluteExternalsRelative, NormalizedBundlerOptions, ResolvedExternal, ResolvedId,
};
use rolldown_error::SingleBuildResult;
use rolldown_resolver::{ResolveError, Resolver};
use std::{path::Path, sync::Arc};
use sugar_path::SugarPath;

use crate::__inner::resolve_id_with_plugins;

use super::resolve_id_with_plugins::is_data_url;

#[expect(clippy::too_many_arguments)]
pub async fn resolve_id_check_external(
  resolver: &Resolver,
  plugin_driver: &PluginDriver,
  specifier: &str,
  importer: Option<&str>,
  is_entry: bool,
  import_kind: ImportKind,
  skipped_resolve_calls: Option<Vec<Arc<HookResolveIdSkipped>>>,
  custom: Arc<CustomField>,
  is_user_defined_entry: bool,
  bundle_options: &NormalizedBundlerOptions,
) -> SingleBuildResult<Result<ResolvedId, ResolveError>> {
  // Check external with unresolved path
  if bundle_options.external.call(specifier, importer, false).await? {
    return Ok(Ok(resolve_external(bundle_options, specifier, importer, true).await?.unwrap()));
  }

  let resolved_id = resolve_id_with_plugins(
    resolver,
    plugin_driver,
    specifier,
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
      if matches!(resolved_id.normalize_external_id, Some(true)) {
        return Ok(Ok(
          resolve_external(bundle_options, &resolved_id.id, importer, true).await?.unwrap(),
        ));
      }
      let external = match resolved_id.external {
        ResolvedExternal::Bool(false) => {
          let specifier = resolved_id.id.as_str();
          bundle_options.external.call(specifier, importer, true).await?.into()
        }
        _ => resolved_id.external,
      };
      resolved_id.external = match external {
        ResolvedExternal::Bool(false) => ResolvedExternal::Bool(false),
        ResolvedExternal::Relative => ResolvedExternal::Bool(true),
        _ => {
          if !is_absolute(resolved_id.id.as_str())
            || (matches!(external, ResolvedExternal::Bool(true))
              && is_not_absolute_external(
                resolved_id.id.as_str(),
                specifier,
                &bundle_options.make_absolute_externals_relative,
              ))
          {
            ResolvedExternal::Bool(true)
          } else {
            ResolvedExternal::Absolute
          }
        }
      };
      Ok(Ok(resolved_id))
    }
    Err(e) => {
      if let ResolveError::NotFound(_) = &e {
        // If module can't resolve, check external with unresolved path with `isResolved: true`
        if let Some(resolved_id) =
          resolve_external(bundle_options, specifier, importer, false).await?
        {
          return Ok(Ok(resolved_id));
        }
      }
      Ok(Err(e))
    }
  }
}

async fn resolve_external(
  options: &NormalizedBundlerOptions,
  specifier: &str,
  importer: Option<&str>,
  is_external: bool,
) -> SingleBuildResult<Option<ResolvedId>> {
  let id = if options.make_absolute_externals_relative.is_enabled() {
    normalize_relative_external_id(&options.cwd, specifier, importer)
  } else {
    specifier.into()
  };

  if !is_external && !options.external.call(&id, importer, true).await? {
    return Ok(None);
  }

  let external =
    if is_not_absolute_external(&id, specifier, &options.make_absolute_externals_relative) {
      ResolvedExternal::Bool(true)
    } else {
      ResolvedExternal::Absolute
    };

  Ok(Some(ResolvedId { id, external, ..Default::default() }))
}

fn is_not_absolute_external(
  id: &str,
  specifier: &str,
  make_absolute_externals_relative: &MakeAbsoluteExternalsRelative,
) -> bool {
  match make_absolute_externals_relative {
    MakeAbsoluteExternalsRelative::Bool(true) => true,
    MakeAbsoluteExternalsRelative::IfRelativeSource if is_relative(specifier) => true,
    _ => !is_absolute(id),
  }
}

fn normalize_relative_external_id(cwd: &Path, specifier: &str, importer: Option<&str>) -> ArcStr {
  if !is_relative(specifier) || importer.is_some_and(is_data_url) {
    return specifier.into();
  }
  let path = if let Some(importer) = importer {
    Path::new(importer).join("..").join(specifier)
  } else {
    cwd.join(specifier)
  };
  path.normalize().to_string_lossy().into()
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
