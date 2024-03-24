use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;
use rolldown_common::ModuleType;
use rolldown_error::BuildError;
use rolldown_plugin::{HookResolveIdArgs, HookResolveIdExtraOptions, SharedPluginDriver};

use crate::{types::resolved_request_info::ResolvedRequestInfo, SharedResolver};

static HTTP_URL_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^(https?:)?\/\/").expect("Init HTTP_URL_REGEX failed"));
static DATA_URL_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^\s*data:").expect("Init DATA_URL_REGEX failed"));

#[allow(clippy::no_effect_underscore_binding)]
pub async fn resolve_id(
  resolver: &SharedResolver,
  plugin_driver: &SharedPluginDriver,
  request: &str,
  importer: Option<&str>,
  options: HookResolveIdExtraOptions,
  _preserve_symlinks: bool,
) -> Result<ResolvedRequestInfo, BuildError> {
  // Run plugin resolve_id first, if it is None use internal resolver as fallback
  if let Some(r) = plugin_driver
    .resolve_id(&HookResolveIdArgs {
      importer: importer.map(std::convert::AsRef::as_ref),
      source: request,
      options,
    })
    .await?
  {
    return Ok(ResolvedRequestInfo {
      path: r.id.into(),
      module_type: ModuleType::Unknown,
      is_external: matches!(r.external, Some(true)),
    });
  }

  // Auto external http url or data url
  if HTTP_URL_REGEX.is_match(request) || DATA_URL_REGEX.is_match(request) {
    return Ok(ResolvedRequestInfo {
      path: request.to_string().into(),
      module_type: ModuleType::Unknown,
      is_external: true,
    });
  }

  // Rollup external node packages by default.
  // Rolldown will follow esbuild behavior to resolve it by default.
  // See https://github.com/rolldown/rolldown/issues/282
  let resolved = resolver.resolve(importer.map(Path::new), request)?;
  Ok(ResolvedRequestInfo {
    path: resolved.resolved,
    module_type: resolved.module_type,
    is_external: false,
  })
}
