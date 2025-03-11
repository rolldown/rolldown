use arcstr::ArcStr;
use rolldown_common::{ResolvedExternal, side_effects::HookSideEffects};

#[derive(Debug, Default)]
pub struct HookResolveIdOutput {
  pub id: ArcStr,
  pub external: Option<ResolvedExternal>,
  pub side_effects: Option<HookSideEffects>,
}
