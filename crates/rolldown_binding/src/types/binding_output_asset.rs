use napi_derive::napi;

#[napi]
pub struct BindingOutputAsset {
  inner: &'static mut rolldown_common::OutputAsset,
}

#[napi]
impl BindingOutputAsset {
  pub fn new(inner: &'static mut rolldown_common::OutputAsset) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn file_name(&self) -> String {
    self.inner.filename.clone()
  }

  #[napi(getter)]
  pub fn source(&self) -> String {
    self.inner.source.clone()
  }

  #[napi(setter, js_name = "source")]
  pub fn set_source(&mut self, source: String) {
    self.inner.source = source;
  }
}
