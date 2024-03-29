use napi_derive::napi;

#[napi]
pub struct BindingOutputAsset {
  inner: Box<rolldown_common::OutputAsset>,
}

#[napi]
impl BindingOutputAsset {
  pub fn new(inner: Box<rolldown_common::OutputAsset>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn file_name(&self) -> String {
    self.inner.file_name.clone()
  }

  #[napi(getter)]
  pub fn source(&self) -> String {
    self.inner.source.clone()
  }
}
