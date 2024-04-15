use anyhow::Result;
use rolldown_common::ResolvedPath;
use rolldown_plugin::{HookTransformArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;

pub async fn transform_source(
  plugin_driver: &PluginDriver,
  resolved_path: &ResolvedPath,
  source: String,
  sourcemap_chain: &mut Vec<SourceMap>,
) -> Result<String> {
  let (code, map_chain) =
    plugin_driver.transform(&HookTransformArgs { id: &resolved_path.path, code: &source }).await?;

  sourcemap_chain.extend(map_chain);

  Ok(code)
}
