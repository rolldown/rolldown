use std::sync::Arc;
use std::sync::mpsc::Sender;

use rolldown_common::ModuleType;
use rolldown_common::SourceMapGenMsg;
use rolldown_common::{
  ModuleIdx, ResolvedId, SourcemapChainElement, side_effects::HookSideEffects,
};
use rolldown_error::SingleBuildResult;
use rolldown_plugin::PluginDriver;

#[inline]
#[tracing::instrument(level = "debug", skip_all)]
#[expect(clippy::too_many_arguments)]
pub async fn transform_source(
  plugin_driver: &PluginDriver,
  resolved_id: &ResolvedId,
  module_idx: ModuleIdx,
  source: String,
  sourcemap_chain: &mut Vec<SourcemapChainElement>,
  side_effects: &mut Option<HookSideEffects>,
  module_type: &mut ModuleType,
  magic_string_tx: Option<Arc<Sender<SourceMapGenMsg>>>,
) -> SingleBuildResult<String> {
  plugin_driver
    .transform(
      &resolved_id.id,
      module_idx,
      source,
      sourcemap_chain,
      side_effects,
      module_type,
      magic_string_tx,
    )
    .await
}
