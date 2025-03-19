use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{RollupRenderedChunk, SharedNormalizedBundlerOptions};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct HookRenderChunkArgs<'a> {
  pub options: &'a SharedNormalizedBundlerOptions,
  pub code: String,
  pub chunk: &'a RollupRenderedChunk,
  pub chunks: Arc<FxHashMap<ArcStr, &'a RollupRenderedChunk>>,
}
