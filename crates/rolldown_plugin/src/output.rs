use rolldown_sourcemap::SourceMap;
#[derive(Debug)]
pub struct HookResolveIdOutput {
  pub id: String,
  pub external: Option<bool>,
}

#[derive(Debug)]
pub struct HookLoadOutput {
  pub code: String,
  pub map: Option<SourceMap>,
}

#[derive(Debug)]
pub struct HookRenderChunkOutput {
  pub code: String,
}
