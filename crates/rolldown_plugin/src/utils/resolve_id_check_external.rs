use crate::{
  PluginDriver,
  types::{custom_field::CustomField, hook_resolve_id_skipped::HookResolveIdSkipped},
};
use arcstr::ArcStr;
use nodejs_built_in_modules::is_nodejs_builtin_module;
use rolldown_common::{
  ImportAttributes, ImportKind, MakeAbsoluteExternalsRelative, ModuleId, NormalizedBundlerOptions,
  ResolvedExternal, ResolvedId,
};
use rolldown_fs::FileSystem;
use rolldown_resolver::{ResolveError, Resolver};
use rolldown_utils::dataurl::is_data_url;
use std::{
  path::{Path, PathBuf},
  sync::Arc,
};
use sugar_path::SugarPath;

use crate::__inner::resolve_id_with_plugins;

/// Resolves `specifier` imported with `attributes`, applying the `external` option. The
/// attributes the import resolves with become part of the module id when
/// `experimental.importAttributes` is enabled.
#[expect(clippy::too_many_arguments)]
pub async fn resolve_id_check_external<Fs: FileSystem>(
  resolver: &Resolver<Fs>,
  plugin_driver: &PluginDriver,
  specifier: &str,
  importer: Option<&str>,
  is_entry: bool,
  import_kind: ImportKind,
  skipped_resolve_calls: Option<Vec<Arc<HookResolveIdSkipped>>>,
  custom: Arc<CustomField>,
  is_user_defined_entry: bool,
  bundle_options: &NormalizedBundlerOptions,
  attributes: &ImportAttributes,
  importer_raw_id: Option<&str>,
  importer_attributes: &ImportAttributes,
) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
  let import_attributes_enabled = bundle_options.experimental.is_import_attributes_enabled();
  let finalize =
    |resolved_id: ResolvedId| resolved_id.finalize_attributes(import_attributes_enabled);
  // Check external with unresolved path. Plugins don't run, so the import keeps the attributes
  // written in the source.
  if bundle_options.external.call(specifier, importer, false).await? {
    let resolved_id =
      resolve_external(bundle_options, specifier, importer, importer_raw_id, true).await?.unwrap();
    return Ok(Ok(finalize(ResolvedId { attributes: attributes.clone(), ..resolved_id })));
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
    attributes,
    importer_raw_id,
    importer_attributes,
  )
  .await?;

  match resolved_id {
    Ok(mut resolved_id) => {
      if matches!(resolved_id.normalize_external_id, Some(true)) {
        let external =
          resolve_external(bundle_options, &resolved_id.id, importer, importer_raw_id, true)
            .await?
            .unwrap();
        return Ok(Ok(finalize(ResolvedId { attributes: resolved_id.attributes, ..external })));
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
      Ok(Ok(finalize(resolved_id)))
    }
    Err(e) => {
      if let ResolveError::NotFound(_) = &e {
        // If module can't resolve, check external with unresolved path with `isResolved: true`
        if let Some(resolved_id) =
          resolve_external(bundle_options, specifier, importer, importer_raw_id, false).await?
        {
          return Ok(Ok(finalize(ResolvedId { attributes: attributes.clone(), ..resolved_id })));
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
  importer_raw_id: Option<&str>,
  is_external: bool,
) -> anyhow::Result<Option<ResolvedId>> {
  let id = if options.make_absolute_externals_relative.is_enabled() {
    normalize_relative_external_id(&options.cwd, specifier, importer_raw_id)
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

  Ok(Some(ResolvedId {
    is_external_without_side_effects: matches!(options.platform, rolldown_common::Platform::Node)
      && is_nodejs_builtin_module(&id),
    id: ModuleId::new(id),
    external,
    ..Default::default()
  }))
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
  if !is_relative(specifier)
    || importer.is_some_and(|id| is_data_url(id) || id.starts_with("\0rolldown/data-url:"))
  {
    return specifier.into();
  }
  let path = if let Some(importer) = importer {
    Path::new(importer).join("..").join(specifier)
  } else {
    cwd.join(specifier)
  };
  path.normalize().components().collect::<PathBuf>().to_string_lossy().into()
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

  #[test]
  fn normalized_external_id_drops_trailing_separator() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
      normalize_relative_external_id(&cwd, "./external", None),
      normalize_relative_external_id(&cwd, "./external/", None)
    );
  }
}
