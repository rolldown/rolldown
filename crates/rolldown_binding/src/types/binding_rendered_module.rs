use napi::bindgen_prelude::FromNapiValue;
use napi_derive::napi;
use rolldown_common::RenderedModule;
use std::fmt::Debug;

#[napi]
#[derive(Clone)]
pub struct BindingRenderedModule {
  #[napi(skip)]
  pub inner: RenderedModule,
}

#[napi]
impl BindingRenderedModule {
  #[napi(getter)]
  pub fn code(&self) -> Option<String> {
    self.inner.code()
  }
}

impl Debug for BindingRenderedModule {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingRenderedModule").field("code", &"...").finish()
  }
}

impl From<rolldown_common::RenderedModule> for BindingRenderedModule {
  fn from(value: rolldown_common::RenderedModule) -> Self {
    Self { inner: value }
  }
}

impl FromNapiValue for BindingRenderedModule {
  unsafe fn from_napi_value(
    _env: napi::sys::napi_env,
    _napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    Ok(BindingRenderedModule { inner: RenderedModule::default() })
  }
}
