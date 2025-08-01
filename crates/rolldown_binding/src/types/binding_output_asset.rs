use std::sync::Arc;

use napi_derive::napi;

use crate::options::plugin::types::binding_asset_source::BindingAssetSource;

#[napi]
pub struct BindingOutputAsset {
  // Shared reference to output asset data for efficient cross-language passing
  inner: Arc<rolldown_common::OutputAsset>,
}

#[napi]
impl BindingOutputAsset {
  // Create binding wrapper for shared asset data
  pub fn new(inner: Arc<rolldown_common::OutputAsset>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn file_name(&self) -> String {
    self.inner.filename.to_string()
  }

  #[napi(getter)]
  pub fn original_file_name(&self) -> Option<String> {
    self.inner.original_file_names.first().cloned()
  }

  #[napi(getter)]
  pub fn original_file_names(&self) -> Vec<String> {
    self.inner.original_file_names.clone()
  }

  #[napi(getter)]
  pub fn source(&self) -> BindingAssetSource {
    self.inner.source.clone().into()
  }

  #[napi(getter)]
  pub fn name(&self) -> Option<String> {
    self.inner.names.first().cloned()
  }

  #[napi(getter)]
  pub fn names(&self) -> Vec<String> {
    self.inner.names.clone()
  }
}

#[napi(object)]
pub struct JsOutputAsset {
  pub names: Vec<String>,
  pub original_file_names: Vec<String>,
  pub filename: String,
  pub source: BindingAssetSource,
}

impl From<JsOutputAsset> for rolldown_common::OutputAsset {
  fn from(asset: JsOutputAsset) -> Self {
    Self {
      names: asset.names,
      original_file_names: asset.original_file_names,
      filename: asset.filename.into(),
      source: asset.source.into(),
    }
  }
}
