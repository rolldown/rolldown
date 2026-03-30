use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{RollupRenderedChunk, SharedNormalizedBundlerOptions};
use rolldown_utils::indexmap::FxIndexMap;

#[derive(Debug)]
pub struct HookRenderChunkArgs<'a> {
  pub options: &'a SharedNormalizedBundlerOptions,
  pub code: String,
  pub chunk: Arc<RollupRenderedChunk>,
  pub chunks: Arc<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>,
}
