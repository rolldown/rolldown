use rolldown_common::{ModuleType, RawPath, ResourceId};
use rolldown_resolver::Resolver;

use crate::BuildResult;

pub struct ResolvedRequestInfo {
  pub path: RawPath,
  pub module_type: ModuleType,
  pub is_external: bool,
}

#[allow(clippy::unused_async)]
pub async fn resolve_id(
  resolver: &Resolver,
  request: &str,
  importer: Option<&ResourceId>,
  _preserve_symlinks: bool,
) -> BuildResult<Option<ResolvedRequestInfo>> {
  // TODO: resolve with plugins

  // external modules (non-entry modules that start with neither '.' or '/')
  // are skipped at this stage.
  if importer.is_some() && !request.starts_with('.') {
    Ok(None)
  } else {
    let resolved = resolver.resolve(importer, request)?;
    Ok(Some(ResolvedRequestInfo {
      path: resolved.resolved,
      module_type: resolved.module_type,
      is_external: false,
    }))
  }
}
