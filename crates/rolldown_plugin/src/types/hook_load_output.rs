use rolldown_sourcemap::SourceMap;

#[derive(Debug)]
pub struct HookLoadOutput {
  pub code: String,
  pub map: Option<SourceMap>,
}
