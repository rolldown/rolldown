use rolldown_common::{ModuleType, RawPath, ResourceId};
use rolldown_error::BuildError;
use rolldown_fs::FileSystemExt;
use rolldown_resolver::Resolver;

pub struct ResolvedRequestInfo {
  pub path: RawPath,
  pub module_type: ModuleType,
  pub is_external: bool,
}

#[allow(clippy::unused_async)]
pub async fn resolve_id<T: FileSystemExt + Default>(
  resolver: &Resolver<T>,
  request: &str,
  importer: Option<&ResourceId>,
  _preserve_symlinks: bool,
) -> Result<Option<ResolvedRequestInfo>, BuildError> {
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
