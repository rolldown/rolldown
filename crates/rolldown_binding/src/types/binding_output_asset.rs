use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingOutputAsset {
  pub file_name: String,
  pub source: String,
}

impl From<Box<rolldown_common::OutputAsset>> for BindingOutputAsset {
  fn from(chunk: Box<rolldown_common::OutputAsset>) -> Self {
    Self { source: chunk.source, file_name: chunk.file_name }
  }
}
