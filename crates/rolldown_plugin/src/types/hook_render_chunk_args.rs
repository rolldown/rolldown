use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{
  ChunkTable, ModuleTable, RollupRenderedChunk, SharedNormalizedBundlerOptions,
};
use rolldown_utils::indexmap::FxIndexMap;

#[derive(Debug)]
pub struct HookRenderChunkArgs<'a> {
  pub options: &'a SharedNormalizedBundlerOptions,
  pub chunk_table: &'a ChunkTable,
  pub module_table: &'a ModuleTable,
  // Keep `String` recoverable after hooks; `Arc<str>` would force a final full clone.
  #[expect(clippy::rc_buffer, reason = "Arc<String> lets renderChunk recover owned code")]
  pub code: Arc<String>,
  pub chunk: Arc<RollupRenderedChunk>,
  pub chunks: Arc<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>,
}

impl HookRenderChunkArgs<'_> {
  pub fn into_code(self) -> String {
    Arc::unwrap_or_clone(self.code)
  }
}
