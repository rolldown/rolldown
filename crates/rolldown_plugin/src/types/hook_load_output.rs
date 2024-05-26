use rolldown_common::side_effects::HookSideEffects;
use rolldown_sourcemap::SourceMap;

#[derive(Debug)]
pub struct HookLoadOutput {
  pub code: String,
  pub map: Option<SourceMap>,
  pub side_effects: Option<HookSideEffects>,
}
