use napi::bindgen_prelude::FromNapiValue;
use napi_derive::napi;
use rolldown_common::RenderedModule;
use std::{fmt::Debug, sync::Arc};

use super::external_memory_status::ExternalMemoryStatus;

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
    match self.inner.take() {
      None => ExternalMemoryStatus {
        freed: false,
        reason: Some("Memory has already been freed".to_string()),
      },
      Some(arc) => {
        let strong_count = Arc::strong_count(&arc);
        if strong_count > 1 {
          ExternalMemoryStatus {
            freed: false,
            reason: Some(format!(
              "Data has been dropped, but there are {} other strong reference(s) referring to this data on the native side, so the memory may not be released.",
              strong_count - 1
            )),
          }
        } else {
          ExternalMemoryStatus { freed: true, reason: None }
        }
      }
    }
  }

  #[napi(getter)]
  pub fn code(&self) -> napi::Result<Option<String>> {
    Ok(self.try_get_inner()?.code())
  }

  // Owned `String`s, not `&str` borrowed from the inner `Arc`: napi-derive
  // converts the `Vec` after this function returns, one `napi_set_element` at a
  // time, so an `Array.prototype` index setter can re-enter `dropInner()`
  // between elements and free the data the remaining borrows point into.
  #[napi(getter)]
  pub fn rendered_exports(&self) -> napi::Result<Vec<String>> {
    Ok(self.try_get_inner()?.rendered_exports.iter().map(ToString::to_string).collect())
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
