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

  #[napi]
  pub fn get_file_name(&self) -> &str {
    &self.inner.filename
  }

  #[napi]
  pub fn get_original_file_name(&self) -> Option<&str> {
    self.inner.original_file_names.first().map(AsRef::as_ref)
  }

  #[napi]
  pub fn get_original_file_names(&self) -> Vec<&str> {
    self.inner.original_file_names.iter().map(AsRef::as_ref).collect()
  }

  #[napi]
  pub fn get_source(&self) -> BindingAssetSource {
    self.inner.source.clone().into()
  }

  #[napi]
  pub fn get_name(&self) -> Option<&str> {
    self.inner.names.first().map(AsRef::as_ref)
  }

  #[napi]
  pub fn get_names(&self) -> Vec<&str> {
    self.inner.names.iter().map(AsRef::as_ref).collect()
  }
}

#[napi_derive::napi(object, object_to_js = false)]
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
