use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingAdvancedChunksOptions {
  // pub share_count: Option<u32>,
  pub groups: Option<Vec<BindingMatchGroup>>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingMatchGroup {
  pub name: String,
  pub test: Option<String>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
}
