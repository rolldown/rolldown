#[derive(Debug)]
pub struct HookResolveIdOutput {
  pub id: String,
  pub external: Option<bool>,
}

#[derive(Debug)]
pub struct HookLoadOutput {
  pub code: String,
}

#[derive(Debug)]
pub struct HookRenderChunkOutput {
  pub code: String,
}
