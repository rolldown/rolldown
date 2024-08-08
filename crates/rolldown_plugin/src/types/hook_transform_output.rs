use rolldown_common::side_effects::HookSideEffects;
use rolldown_sourcemap::SourceMapOrMissing;

#[derive(Debug, Default)]
pub struct HookTransformOutput {
  pub code: Option<String>,
  pub map: Option<SourceMapOrMissing>,
  pub side_effects: Option<HookSideEffects>,
}
