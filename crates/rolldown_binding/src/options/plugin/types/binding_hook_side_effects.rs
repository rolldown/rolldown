use napi_derive::napi;
use rolldown_common::side_effects::HookSideEffects;

#[derive(Debug, PartialEq)]
#[napi]
pub enum BindingHookSideEffects {
  True,
  False,
  NoTreeshake,
}

impl From<BindingHookSideEffects> for HookSideEffects {
  fn from(value: BindingHookSideEffects) -> Self {
    match value {
      BindingHookSideEffects::True => Self::True,
      BindingHookSideEffects::False => Self::False,
      BindingHookSideEffects::NoTreeshake => Self::NoTreeshake,
    }
  }
}

impl From<HookSideEffects> for BindingHookSideEffects {
  fn from(value: HookSideEffects) -> Self {
    match value {
      HookSideEffects::True => Self::True,
      HookSideEffects::False => Self::False,
      HookSideEffects::NoTreeshake => Self::NoTreeshake,
    }
  }
}
