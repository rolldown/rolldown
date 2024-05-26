use derivative::Derivative;
use napi_derive::napi;
use serde::Deserialize;

#[derive(Deserialize, Derivative, Debug, PartialEq)]
#[napi]
pub enum BindingHookSideEffects {
  True,
  False,
  NoTreeshake,
}

impl From<BindingHookSideEffects> for rolldown_common::side_effects::HookSideEffects {
  fn from(value: BindingHookSideEffects) -> Self {
    match value {
      BindingHookSideEffects::True => Self::True,
      BindingHookSideEffects::False => Self::False,
      BindingHookSideEffects::NoTreeshake => Self::NoTreeshake,
    }
  }
}
