use rolldown_common::side_effects::HookSideEffects;

#[derive(Debug, Default)]
pub struct HookResolveIdOutput {
  pub id: String,
  pub external: Option<bool>,
  pub side_effects: Option<HookSideEffects>,
}
