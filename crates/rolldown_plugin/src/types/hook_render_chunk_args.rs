use rolldown_common::RollupRenderedChunk;

#[derive(Debug)]
pub struct HookRenderChunkArgs<'a> {
  pub code: String,
  pub chunk: &'a RollupRenderedChunk,
}
