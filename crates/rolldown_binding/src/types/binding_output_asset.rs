use napi_derive::napi;

use crate::options::plugin::types::binding_asset_source::BindingAssetSource;

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
    self.inner.filename.to_string()
  }

  #[napi(getter)]
  pub fn original_file_name(&self) -> Option<String> {
    self.inner.original_file_name.clone()
  }

  #[napi(getter)]
  pub fn source(&self) -> BindingAssetSource {
    self.inner.source.clone().into()
  }

  #[napi(setter, js_name = "source")]
  pub fn set_source(&mut self, source: BindingAssetSource) {
    self.inner.source = source.into();
  }

  #[napi(getter)]
  pub fn name(&self) -> Option<String> {
    self.inner.name.clone()
  }
}
