use rolldown_plugin::HookTransformArgs;
use rolldown_sourcemap::SourceMap;

use crate::{error::BatchedErrors, plugin_driver::PluginDriver};

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
