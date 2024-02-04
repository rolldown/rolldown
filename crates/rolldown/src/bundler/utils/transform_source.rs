use rolldown_sourcemap::SourceMap;

use crate::{bundler::plugin_driver::PluginDriver, error::BatchedErrors, HookTransformArgs};

pub async fn transform_source(
  plugin_driver: &PluginDriver,
  source: String,
  sourcemap_chain: &mut Vec<SourceMap>,
) -> Result<String, BatchedErrors> {
  let (code, map_chain) =
    plugin_driver.transform(&HookTransformArgs { id: source.as_ref(), code: &source }).await?;

  sourcemap_chain.extend(map_chain);

  Ok(code)
}
