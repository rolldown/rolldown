use std::fmt::Debug;

use napi_derive::napi;

#[napi(object)]
pub struct BindingRenderedModule {
  pub code: Option<String>,
}

impl Debug for BindingRenderedModule {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingRenderedModule").field("code", &"...").finish()
  }
}

impl From<rolldown_common::RenderedModule> for BindingRenderedModule {
  fn from(value: rolldown_common::RenderedModule) -> Self {
    Self { code: value.code }
  }
}
