use arcstr::ArcStr;
use rolldown_common::{ModuleType, side_effects::HookSideEffects};
use rolldown_sourcemap::SourceMap;

#[derive(Debug, Default)]
pub struct HookLoadOutput {
  pub code: ArcStr,
  pub map: Option<SourceMap>,
  pub side_effects: Option<HookSideEffects>,
  pub module_type: Option<ModuleType>,
}
