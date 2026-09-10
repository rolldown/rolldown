use std::sync::Arc;

use napi_derive::napi;

use rolldown_plugin::SharedLoadPluginContext;

use super::binding_plugin_context::BindingPluginContext;
use crate::types::external_memory_status::ExternalMemoryStatus;

#[napi]
pub struct BindingLoadPluginContext {
  inner: Option<SharedLoadPluginContext>,
}

#[napi]
impl BindingLoadPluginContext {
  pub fn new(inner: SharedLoadPluginContext) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&SharedLoadPluginContext> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed: this load context's native data was eagerly released after its hook invocation settled. Use the context only while the hook runs.",
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

  #[napi]
  pub fn inner(&self) -> napi::Result<BindingPluginContext> {
    Ok(self.try_get_inner()?.inner.clone().into())
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) -> napi::Result<()> {
    self.try_get_inner()?.add_watch_file(&file);
    Ok(())
  }
}
