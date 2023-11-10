use rolldown_common::{ModuleType, RawPath, ResourceId};
use rolldown_error::BuildError;
use rolldown_fs::FileSystemExt;
use rolldown_resolver::Resolver;

use crate::{bundler::plugin_driver::SharedPluginDriver, HookResolveIdArgs};

#[derive(Debug)]
pub struct ResolvedRequestInfo {
  pub path: RawPath,
  pub module_type: ModuleType,
  pub is_external: bool,
}

#[allow(clippy::unused_async)]
pub async fn resolve_id<T: FileSystemExt + Default>(
  resolver: &Resolver<T>,
  plugin_driver: &SharedPluginDriver,
  request: &str,
  importer: Option<&ResourceId>,
  _preserve_symlinks: bool,
) -> Result<Option<ResolvedRequestInfo>, BuildError> {
  // Run plugin resolve_id first, if it is None use internal resolver as fallback
  if let Some(r) = plugin_driver
    .resolve_id(&HookResolveIdArgs {
      importer: importer.map(std::convert::AsRef::as_ref),
      source: request,
    })
    .await?
  {
    return Ok(Some(ResolvedRequestInfo {
      path: r.id.into(),
      module_type: ModuleType::Unknown,
      is_external: matches!(r.external, Some(true)),
    }));
  }

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
