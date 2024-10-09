use derivative::Derivative;
use napi::Either;
use serde::Deserialize;

use crate::options::plugin::types::binding_js_or_regex::JsRegExp;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingAdvancedChunksOptions {
  pub min_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub groups: Option<Vec<BindingMatchGroup>>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingMatchGroup {
  pub name: String,
  #[napi(ts_type = "RegExp | String")]
  #[serde(skip_deserializing)]
  pub test: Option<Either<JsRegExp, String>>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
  pub min_size: Option<f64>,
  pub min_share_count: Option<u32>,
}
