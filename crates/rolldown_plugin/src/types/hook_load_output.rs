use rolldown_common::{side_effects::HookSideEffects, ModuleType};
use rolldown_sourcemap::SourceMapOrMissing;

#[derive(Debug, Default)]
pub struct HookLoadOutput {
  pub code: String,
  pub map: Option<SourceMapOrMissing>,
  pub side_effects: Option<HookSideEffects>,
  pub module_type: Option<ModuleType>,
}
