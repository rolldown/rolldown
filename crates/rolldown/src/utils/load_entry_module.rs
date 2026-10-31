use std::sync::Arc;

use crate::SharedResolver;
use crate::utils::resolve_id::resolve_id;
use rolldown_common::{ImportAttributes, ImportKind, NormalizedBundlerOptions, ResolvedId};
use rolldown_error::ResultExt;
use rolldown_error::{BuildDiagnostic, SingleBuildResult};
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_resolver::ResolveError;

pub async fn load_entry_module<Fs: FileSystem>(
  options: &NormalizedBundlerOptions,
  resolver: &SharedResolver<Fs>,
  plugin_driver: &SharedPluginDriver,
  id: &str,
  importer: Option<&str>,
) -> SingleBuildResult<ResolvedId> {
  let importer_raw_id = importer.map(|importer| {
    ImportAttributes::parse_module_id(importer).map_or(importer, |(raw_id, _)| raw_id)
  });
  let result = resolve_id(
    resolver,
    plugin_driver,
    id,
    importer,
    true,
    ImportKind::Import,
    None,
    Arc::default(),
    true,
    &ImportAttributes::default(),
    importer_raw_id,
  )
  .await?;

  match result {
    Ok(result) => {
      let result = result.finalize_attributes(options.experimental.is_import_attributes_enabled());
      if result.external.is_external() {
        Err(BuildDiagnostic::entry_cannot_be_external(result.id.as_str()))
      } else {
        Ok(result)
      }
    }
    Err(e) => match e {
      ResolveError::NotFound(_) => Err(BuildDiagnostic::unresolved_entry(id, None)),
      ResolveError::PackagePathNotExported {
        subpath: _,
        package_path: _,
        package_json_path: _,
        conditions: _,
      } => Err(BuildDiagnostic::unresolved_entry(id, Some(e))),
      ResolveError::TsconfigNotFound(_)
      | ResolveError::TsconfigSelfReference(_)
      | ResolveError::TsconfigCircularExtend(_)
      | ResolveError::TsconfigLoadFailed { .. } => Err(BuildDiagnostic::tsconfig_error(e)),
      _ => Err(e).map_err_to_unhandleable()?,
    },
  }
}
