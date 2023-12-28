use napi_derive::napi;

#[derive(Debug)]
#[napi]
pub struct PluginContext {
  inner: &'static rolldown::PluginContext,
}

#[napi]
impl PluginContext {
  #[napi]
  pub fn load(&self) {
    self.inner.load();
  }
}

impl<'a> From<&'a rolldown::PluginContext> for PluginContext {
  fn from(inner: &'a rolldown::PluginContext) -> Self {
    unsafe { Self { inner: std::mem::transmute(inner) } }
  }
}

#[derive(Debug)]
#[napi]
pub struct TransformPluginContext {
  inner: &'static rolldown::TransformPluginContext<'static>,
}

#[napi]
impl TransformPluginContext {
  #[napi]
  pub fn get_ctx(&self) -> PluginContext {
    self.inner.inner.into()
  }
}

impl<'a> From<&'a rolldown::TransformPluginContext<'_>> for TransformPluginContext {
  fn from(inner: &'a rolldown::TransformPluginContext) -> Self {
    unsafe { Self { inner: std::mem::transmute(inner) } }
  }
}
