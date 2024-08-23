use derivative::Derivative;
use serde::Deserialize;

use super::binding_asset_source::BindingAssetSource;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[derivative(Debug)]
pub struct BindingEmittedAsset {
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub original_file_name: Option<String>,
  pub source: BindingAssetSource,
}

impl From<BindingEmittedAsset> for rolldown_common::EmittedAsset {
  fn from(value: BindingEmittedAsset) -> Self {
    Self {
      name: value.name,
      file_name: value.file_name,
      source: value.source.into(),
      original_file_name: value.original_file_name,
    }
  }
}
