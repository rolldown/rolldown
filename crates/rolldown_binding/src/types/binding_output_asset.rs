use std::sync::Arc;

use napi_derive::napi;

#[napi]
pub struct BindingOutputAsset {
  inner: Arc<rolldown_common::OutputAsset>,
}

#[napi]
impl BindingOutputAsset {
  pub fn new(inner: Arc<rolldown_common::OutputAsset>) -> Self {
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
