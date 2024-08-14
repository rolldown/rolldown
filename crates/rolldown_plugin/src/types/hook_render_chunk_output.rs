use rolldown_sourcemap::SourceMapOrMissing;

#[derive(Debug)]
pub struct HookRenderChunkOutput {
  pub code: String,
  pub map: Option<SourceMapOrMissing>,
}
