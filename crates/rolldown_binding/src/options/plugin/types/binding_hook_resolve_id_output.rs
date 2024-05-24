use derivative::Derivative;
use serde::Deserialize;

use super::binding_hook_side_effects::BindingHookSideEffects;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookResolveIdOutput {
  pub id: String,
  pub external: Option<bool>,
  pub side_effects: Option<BindingHookSideEffects>,
}

impl From<BindingHookResolveIdOutput> for rolldown_plugin::HookResolveIdOutput {
  fn from(value: BindingHookResolveIdOutput) -> Self {
    Self {
      id: value.id,
      external: value.external,
      side_effects: value.side_effects.map(Into::into),
    }
  }
}
