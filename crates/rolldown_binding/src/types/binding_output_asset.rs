use napi_derive::napi;

use crate::options::plugin::types::binding_asset_source::BindingAssetSource;

#[napi]
pub struct BindingOutputAsset {
  inner: rolldown_common::OutputAsset,
}

#[napi]
impl BindingOutputAsset {
  pub fn new(inner: rolldown_common::OutputAsset) -> Self {
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

  #[napi(getter)]
  pub fn name(&self) -> Option<String> {
    self.inner.name.clone()
  }
}

#[napi(object)]
pub struct JsOutputAsset {
  pub name: Option<String>,
  pub original_file_name: Option<String>,
  pub filename: String,
  pub source: BindingAssetSource,
}

impl From<JsOutputAsset> for rolldown_common::OutputAsset {
  fn from(asset: JsOutputAsset) -> Self {
    Self {
      name: asset.name,
      original_file_name: asset.original_file_name,
      filename: asset.filename.into(),
      source: asset.source.into(),
    }
  }
}
