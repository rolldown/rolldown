use std::path::Path;

use rolldown_common::{ImportKind, ModuleType, ResolvedRequestInfo};
use rolldown_resolver::{ResolveError, Resolver};

use crate::{
  HookResolveDynamicImportArgs, HookResolveIdArgs, HookResolveIdExtraOptions, PluginDriver,
};

fn is_http_url(s: &str) -> bool {
  s.starts_with("http://") || s.starts_with("https://") || s.starts_with("//")
}

fn is_data_url(s: &str) -> bool {
  s.trim_start().starts_with("data:")
}

pub async fn resolve_id_with_plugins(
  resolver: &Resolver,
  plugin_driver: &PluginDriver,
  request: &str,
  importer: Option<&str>,
  options: HookResolveIdExtraOptions,
) -> anyhow::Result<Result<ResolvedRequestInfo, ResolveError>> {
  let import_kind = options.kind;
  if matches!(import_kind, ImportKind::DynamicImport) {
    if let Some(r) = plugin_driver
      .resolve_dynamic_import(&HookResolveDynamicImportArgs {
        importer: importer.map(std::convert::AsRef::as_ref),
        source: request,
      })
      .await?
    {
      return Ok(Ok(ResolvedRequestInfo {
        module_type: ModuleType::from_path(&r.id),
        path: r.id.into(),
        is_external: matches!(r.external, Some(true)),
      }));
    }
  }
  // Run plugin resolve_id first, if it is None use internal resolver as fallback
  if let Some(r) = plugin_driver
    .resolve_id(&HookResolveIdArgs {
      importer: importer.map(std::convert::AsRef::as_ref),
      source: request,
      options,
    })
    .await?
  {
    return Ok(Ok(ResolvedRequestInfo {
      module_type: ModuleType::from_path(&r.id),
      path: r.id.into(),
      is_external: matches!(r.external, Some(true)),
    }));
  }

  // Auto external http url or data url
  if is_http_url(request) || is_data_url(request) {
    return Ok(Ok(ResolvedRequestInfo {
      path: request.to_string().into(),
      module_type: ModuleType::Unknown,
      is_external: true,
    }));
  }

  // Rollup external node packages by default.
  // Rolldown will follow esbuild behavior to resolve it by default.
  // See https://github.com/rolldown/rolldown/issues/282
  let resolved = resolver.resolve(importer.map(Path::new), request, import_kind)?;
  Ok(resolved.map(|resolved| ResolvedRequestInfo {
    path: resolved.path,
    module_type: resolved.module_type,
    is_external: false,
  }))
}
