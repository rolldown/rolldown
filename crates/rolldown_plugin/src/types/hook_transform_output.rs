use rolldown_common::side_effects::HookSideEffects;
use rolldown_sourcemap::SourceMap;

#[derive(Debug, Default)]
pub struct HookTransformOutput {
  pub code: Option<String>,
  pub map: Option<SourceMap>,
  pub side_effects: Option<HookSideEffects>,
}
