use rolldown_plugin::SharedPluginContext;

#[napi_derive::napi]
pub struct BindingPluginContext {
  #[allow(dead_code)]
  inner: SharedPluginContext,
}

impl From<SharedPluginContext> for BindingPluginContext {
  fn from(inner: SharedPluginContext) -> Self {
    Self { inner }
  }
}
