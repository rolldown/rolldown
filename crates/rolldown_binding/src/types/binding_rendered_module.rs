use napi::bindgen_prelude::FromNapiValue;
use napi_derive::napi;
use rolldown_common::RenderedModule;
use std::{fmt::Debug, sync::Arc};

#[napi]
#[derive(Clone)]
pub struct BindingRenderedModule {
  inner: Arc<RenderedModule>,
}

#[napi]
impl BindingRenderedModule {
  pub fn new(inner: Arc<RenderedModule>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn code(&self) -> Option<String> {
    self.inner.code()
  }

  #[napi(getter)]
  pub fn rendered_exports(&self) -> Vec<String> {
    self.inner.rendered_exports.iter().map(std::string::ToString::to_string).collect()
  }
}

impl Debug for BindingRenderedModule {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingRenderedModule").field("code", &"...").finish()
  }
}

impl FromNapiValue for BindingRenderedModule {
  unsafe fn from_napi_value(
    _env: napi::sys::napi_env,
    _napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    Ok(BindingRenderedModule { inner: Arc::new(RenderedModule::default()) })
  }
}
