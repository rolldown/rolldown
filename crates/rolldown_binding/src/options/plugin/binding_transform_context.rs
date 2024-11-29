use napi_derive::napi;

use rolldown_plugin::SharedTransformPluginContext;

use super::binding_plugin_context::BindingPluginContext;

#[napi]
pub struct BindingTransformPluginContext {
  inner: SharedTransformPluginContext,
}

#[napi]
impl BindingTransformPluginContext {
  pub fn new(inner: SharedTransformPluginContext) -> Self {
    Self { inner }
  }

  #[napi]
  pub fn get_combined_sourcemap(&self) -> String {
    self.inner.get_combined_sourcemap().to_json_string()
  }

  #[napi]
  pub fn inner(&self) -> BindingPluginContext {
    self.inner.inner.clone().into()
  }
}
