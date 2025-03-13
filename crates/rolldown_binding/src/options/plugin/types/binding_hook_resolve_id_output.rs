use super::{
  binding_hook_side_effects::BindingHookSideEffects,
  binding_resolved_external::BindingResolvedExternal,
};

#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingHookResolveIdOutput {
  pub id: String,
  pub external: Option<BindingResolvedExternal>,
  pub normalize_external_id: Option<bool>,
  pub side_effects: Option<BindingHookSideEffects>,
}

impl From<BindingHookResolveIdOutput> for rolldown_plugin::HookResolveIdOutput {
  fn from(value: BindingHookResolveIdOutput) -> Self {
    Self {
      id: value.id.into(),
      external: value.external.map(Into::into),
      normalize_external_id: value.normalize_external_id,
      side_effects: value.side_effects.map(Into::into),
    }
  }
}
