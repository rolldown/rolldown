use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[derivative(Debug)]
pub struct BindingEmittedAsset {
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub source: String,
}

impl From<BindingEmittedAsset> for rolldown_common::EmittedAsset {
  fn from(value: BindingEmittedAsset) -> Self {
    Self { name: value.name, file_name: value.file_name, source: value.source }
  }
}
