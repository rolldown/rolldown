use rolldown_sourcemap::SourceMap;

#[derive(Debug)]
pub struct HookRenderChunkOutput {
  pub code: String,
  pub map: Option<SourceMap>,
}
