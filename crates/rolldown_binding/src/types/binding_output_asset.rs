use std::sync::{Arc, Weak};

use napi_derive::napi;
use rolldown_common::OutputAsset;

use crate::options::plugin::types::binding_asset_source::BindingAssetSource;

#[napi]
pub struct BindingOutputAsset {
  inner: Weak<OutputAsset>,
}

#[napi]
impl BindingOutputAsset {
  pub fn new(inner: Weak<OutputAsset>) -> Self {
    Self { inner }
  }

  fn inner(&self) -> Arc<OutputAsset> {
    self.inner.upgrade().unwrap()
  }

  #[napi(getter)]
  pub fn file_name(&self) -> String {
    self.inner().filename.to_string()
  }

  #[napi(getter)]
  pub fn original_file_name(&self) -> Option<String> {
    self.inner().original_file_names.first().cloned()
  }

  #[napi(getter)]
  pub fn original_file_names(&self) -> Vec<String> {
    self.inner().original_file_names.clone()
  }

  #[napi(getter)]
  pub fn source(&self) -> BindingAssetSource {
    self.inner().source.clone().into()
  }

  #[napi(getter)]
  pub fn name(&self) -> Option<String> {
    self.inner().names.first().cloned()
  }

  #[napi(getter)]
  pub fn names(&self) -> Vec<String> {
    self.inner().names.clone()
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
