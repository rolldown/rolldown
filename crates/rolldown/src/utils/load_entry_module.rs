use std::sync::Arc;

use crate::SharedResolver;
use crate::utils::resolve_id::resolve_id;
use rolldown_common::{ImportKind, ResolvedId};
use rolldown_error::ResultExt;
use rolldown_error::{BuildDiagnostic, SingleBuildResult};
use rolldown_plugin::SharedPluginDriver;
use rolldown_resolver::ResolveError;

pub async fn load_entry_module(
  resolver: &SharedResolver,
  plugin_driver: &SharedPluginDriver,
  id: &str,
  importer: Option<&str>,
) -> SingleBuildResult<ResolvedId> {
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
  )
  .await?;

  match result {
    Ok(result) => {
      if result.external.is_external() {
        Err(BuildDiagnostic::entry_cannot_be_external(result.id.as_str()))
      } else {
        Ok(result)
      }
    }
    Err(e) => match e {
      ResolveError::NotFound(_) => Err(BuildDiagnostic::unresolved_entry(id, None)),
      ResolveError::PackagePathNotExported(..) => {
        Err(BuildDiagnostic::unresolved_entry(id, Some(e)))
      }
      _ => Err(e).map_err_to_unhandleable()?,
    },
  }
}
