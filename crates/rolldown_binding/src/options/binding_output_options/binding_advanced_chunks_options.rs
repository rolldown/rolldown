use derive_more::Debug;
use napi::{Either, bindgen_prelude::FnArgs};
use rolldown::ChunkingContext;

use crate::types::{
  binding_module_info::BindingModuleInfo, binding_string_or_regex::BindingStringOrRegex,
  js_callback::JsCallback,
};

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
  #[napi(ts_type = "string | ((id: string, ctx: BindingChunkingContext) => VoidNullable<string>)")]
  #[debug("MatchGroupName(...)")]
  pub name: Either<String, JsCallback<FnArgs<(String, BindingChunkingContext)>, Option<String>>>,
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

#[napi_derive::napi]
#[derive(Debug)]
pub struct BindingChunkingContext {
  inner: ChunkingContext,
}

impl BindingChunkingContext {
  pub fn new(inner: ChunkingContext) -> Self {
    Self { inner }
  }
}

#[napi_derive::napi]
impl BindingChunkingContext {
  #[napi]
  pub fn get_module_info(&self, module_id: String) -> Option<BindingModuleInfo> {
    self.inner.get_module_info(&module_id).map(BindingModuleInfo::new)
  }
}
