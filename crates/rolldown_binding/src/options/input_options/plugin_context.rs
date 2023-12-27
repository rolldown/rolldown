use napi_derive::napi;

#[derive(Debug)]
#[napi]
pub struct PluginContext {
  inner: &'static rolldown::PluginContext,
}

#[napi]
impl PluginContext {
  #[napi]
  pub async fn load(&self) -> napi::Result<()> {
    self.inner.load();
    Ok(())
  }
}

impl<'a> From<&'a rolldown::PluginContext> for PluginContext {
  fn from(inner: &'a rolldown::PluginContext) -> Self {
    unsafe { Self { inner: std::mem::transmute(inner) } }
  }
}
