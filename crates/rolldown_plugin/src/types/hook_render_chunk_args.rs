use rolldown_common::RenderedChunk;

#[derive(Debug)]
pub struct HookRenderChunkArgs<'a> {
  pub code: String,
  pub chunk: &'a RenderedChunk,
}
