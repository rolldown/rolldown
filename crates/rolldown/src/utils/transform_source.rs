use anyhow::Result;
use rolldown_common::ModuleType;
use rolldown_common::{side_effects::HookSideEffects, ResolvedId};
use rolldown_plugin::PluginDriver;
use rolldown_sourcemap::SourceMap;

#[inline]
pub async fn transform_source(
  plugin_driver: &PluginDriver,
  resolved_id: &ResolvedId,
  source: String,
  sourcemap_chain: &mut Vec<SourceMap>,
  side_effects: &mut Option<HookSideEffects>,
  module_type: &mut ModuleType,
) -> Result<String> {
  plugin_driver.transform(&resolved_id.id, source, sourcemap_chain, side_effects, module_type).await
}
