use std::sync::Arc;

use napi_derive::napi;

use crate::options::plugin::types::binding_asset_source::BindingAssetSource;

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
  pub fn file_name(&self) -> &str {
    &self.inner.filename
  }

  #[napi(getter)]
  pub fn original_file_name(&self) -> Option<&str> {
    self.inner.original_file_names.first().map(AsRef::as_ref)
  }

  #[napi(getter)]
  pub fn original_file_names(&self) -> Vec<&str> {
    self.inner.original_file_names.iter().map(AsRef::as_ref).collect()
  }

  #[napi(getter)]
  pub fn source(&self) -> BindingAssetSource {
    self.inner.source.clone().into()
  }

  #[napi(getter)]
  pub fn name(&self) -> Option<&str> {
    self.inner.names.first().map(AsRef::as_ref)
  }

  #[napi(getter)]
  pub fn names(&self) -> Vec<&str> {
    self.inner.names.iter().map(AsRef::as_ref).collect()
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
