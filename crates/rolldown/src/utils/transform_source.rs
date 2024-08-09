use anyhow::Result;
use rolldown_common::ModuleType;
use rolldown_common::{side_effects::HookSideEffects, ResolvedId};
use rolldown_plugin::{HookTransformArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;

pub async fn transform_source(
  plugin_driver: &PluginDriver,
  resolved_id: &ResolvedId,
  source: String,
  sourcemap_chain: &mut Vec<SourceMap>,
  side_effects: &mut Option<HookSideEffects>,
  module_type: &mut ModuleType,
) -> Result<String> {
  plugin_driver
    .transform(
      &HookTransformArgs { id: &resolved_id.id, code: &source, module_type: &ModuleType::Empty },
      sourcemap_chain,
      side_effects,
      &source,
      module_type,
    )
    .await
}
