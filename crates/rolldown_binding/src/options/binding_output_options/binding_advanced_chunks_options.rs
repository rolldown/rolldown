use derive_more::Debug;
use napi::{Either, bindgen_prelude::FnArgs};

use crate::types::{binding_string_or_regex::BindingStringOrRegex, js_callback::JsCallback};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingAdvancedChunksOptions {
  pub min_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub groups: Option<Vec<BindingMatchGroup>>,
  pub max_size: Option<f64>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
}

type BindingMatchGroupTest =
  Either<BindingStringOrRegex, JsCallback<FnArgs<(/*module id*/ String,)>, Option<bool>>>;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingMatchGroup {
  #[napi(ts_type = "string | ((id: string) => VoidNullable<string>)")]
  #[debug("MatchGroupName(...)")]
  pub name: Either<String, JsCallback<FnArgs<(String,)>, Option<String>>>,
  #[napi(ts_type = "string | RegExp | ((id: string) => VoidNullable<boolean>)")]
  #[debug("MatchGroupTest(...)")]
  pub test: Option<BindingMatchGroupTest>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
  pub min_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
  pub max_size: Option<f64>,
}
