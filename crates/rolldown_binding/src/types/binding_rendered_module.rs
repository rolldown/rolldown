use napi::bindgen_prelude::FromNapiValue;
use napi_derive::napi;
use rolldown_common::RenderedModule;
use std::{fmt::Debug, sync::Arc};

use super::external_memory_status::{ExternalMemoryStatus, release_arc};

#[napi]
#[derive(Clone)]
pub struct BindingRenderedModule {
  inner: Option<Arc<RenderedModule>>,
}

#[napi]
impl BindingRenderedModule {
  pub fn new(inner: Arc<RenderedModule>) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&Arc<RenderedModule>> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed by `freeExternalMemory()`. Cannot access properties. To prevent this, use `freeExternalMemory(handle, true)` with `keepDataAlive`.",
      )
    })
  }

  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self) -> ExternalMemoryStatus {
    release_arc(&mut self.inner)
  }

  #[napi(getter)]
  pub fn code(&self) -> napi::Result<Option<String>> {
    Ok(self.try_get_inner()?.code())
  }

  #[napi(getter)]
  pub fn rendered_exports(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.rendered_exports.iter().map(AsRef::as_ref).collect())
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
    Ok(BindingRenderedModule { inner: Some(Arc::new(RenderedModule::default())) })
  }
}
