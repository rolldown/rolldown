use once_cell::sync::Lazy;
use regex::Regex;
use rolldown_common::{FilePath, ModuleType};
use rolldown_error::BuildError;
use rolldown_fs::FileSystem;
use rolldown_resolver::Resolver;

use crate::{
  bundler::{options::input_options::SharedInputOptions, plugin_driver::SharedPluginDriver},
  error::BatchedResult,
  HookResolveIdArgs, HookResolveIdArgsOptions,
};

static HTTP_URL_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^(https?:)?\/\/").expect("Init HTTP_URL_REGEX failed"));
static DATA_URL_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^\s*data:").expect("Init DATA_URL_REGEX failed"));

#[derive(Debug)]
pub struct ResolvedRequestInfo {
  pub path: FilePath,
  pub module_type: ModuleType,
  pub is_external: bool,
}

#[allow(clippy::unused_async)]
pub async fn resolve_id<T: FileSystem + Default + 'static>(
  resolver: &Resolver<T>,
  plugin_driver: &SharedPluginDriver<T>,
  request: &str,
  importer: Option<&FilePath>,
  options: HookResolveIdArgsOptions,
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
  // See https://github.com/rolldown-rs/rolldown/issues/282
  let resolved = resolver.resolve(importer, request)?;
  Ok(ResolvedRequestInfo {
    path: resolved.resolved,
    module_type: resolved.module_type,
    is_external: false,
  })
}

#[allow(clippy::option_if_let_else)]
pub async fn resolve_id_without_defaults<T: FileSystem + Default + 'static>(
  input_options: &SharedInputOptions,
  resolver: &Resolver<T>,
  plugin_driver: &SharedPluginDriver<T>,
  importer: Option<FilePath>,
  specifier: &str,
  options: HookResolveIdArgsOptions,
) -> BatchedResult<ResolvedRequestInfo> {
  // Check external with unresolved path
  if input_options
    .external
    .call(specifier.to_string(), importer.as_ref().map(|v| v.to_string()), false)
    .await?
  {
    return Ok(ResolvedRequestInfo {
      path: specifier.to_string().into(),
      module_type: ModuleType::Unknown,
      is_external: true,
    });
  }

  let mut info =
    resolve_id(resolver, plugin_driver, specifier, importer.as_ref(), options, false).await?;

  if !info.is_external {
    // Check external with resolved path
    info.is_external = input_options
      .external
      .call(specifier.to_string(), importer.map(|v| v.to_string()), true)
      .await?;
  }
  Ok(info)
}
