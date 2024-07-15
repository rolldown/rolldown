use rolldown_common::{side_effects::HookSideEffects, ModuleType};
use rolldown_sourcemap::SourceMap;

#[derive(Debug, Default)]
pub struct HookLoadOutput {
  pub code: String,
  pub map: Option<SourceMap>,
  pub side_effects: Option<HookSideEffects>,
  pub module_type: Option<ModuleType>,
}
