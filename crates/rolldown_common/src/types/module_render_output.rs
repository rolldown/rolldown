use rolldown_sourcemap::SourceMap;

#[derive(Debug)]
pub struct ModuleRenderOutput {
  pub code: String,
  pub map: Option<SourceMap>,
}
