use napi_derive::napi;

use rolldown_plugin::SharedLoadPluginContext;

use super::binding_plugin_context::BindingPluginContext;

#[napi]
pub struct BindingLoadPluginContext {
  inner: SharedLoadPluginContext,
}

#[napi]
impl BindingLoadPluginContext {
  pub fn new(inner: SharedLoadPluginContext) -> Self {
    Self { inner }
  }

  #[napi]
  pub fn inner(&self) -> BindingPluginContext {
    self.inner.inner.clone().into()
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) {
    self.inner.add_watch_file(&file);
  }
}
