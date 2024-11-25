use rolldown_common::{RollupRenderedChunk, SharedNormalizedBundlerOptions};

#[derive(Debug)]
pub struct HookRenderChunkArgs<'a> {
  pub options: &'a SharedNormalizedBundlerOptions,
  pub code: String,
  pub chunk: &'a RollupRenderedChunk,
}
