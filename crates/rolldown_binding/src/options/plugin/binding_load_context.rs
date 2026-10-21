use napi_derive::napi;

use rolldown_plugin::SharedLoadPluginContext;

use super::binding_plugin_context::BindingPluginContext;
use crate::types::external_memory_status::{ExternalMemoryStatus, release_arc};

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
    release_arc(&mut self.inner)
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
