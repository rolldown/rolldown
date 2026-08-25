use derive_more::Debug;
use napi::{
  Either,
  bindgen_prelude::{FnArgs, Uint8Array},
};
use rolldown::ChunkingContext;

use crate::types::{
  binding_module_info::BindingModuleInfo, binding_string_or_regex::BindingStringOrRegex,
  js_callback::JsCallback,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingManualCodeSplittingOptions {
  pub include_dependencies_recursively: Option<bool>,
  pub min_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub groups: Option<Vec<BindingMatchGroup>>,
  pub max_size: Option<f64>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
}

/// The JS side wraps the user's per-id function in one shim per group. The result holds one byte
/// per id, and a nonzero byte captures the module. See
/// `packages/rolldown/src/utils/bindingify-output-options.ts`.
type BindingMatchGroupTest =
  Either<BindingStringOrRegex, JsCallback<FnArgs<(/*module ids*/ Vec<String>,)>, Uint8Array>>;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingMatchGroup {
  #[napi(
    ts_type = "string | ((ids: Array<string>, ctx: BindingChunkingContext) => Array<VoidNullable<string>>)"
  )]
  #[debug("MatchGroupName(...)")]
  pub name:
    Either<String, JsCallback<FnArgs<(Vec<String>, BindingChunkingContext)>, Vec<Option<String>>>>,
  #[napi(ts_type = "string | RegExp | ((ids: Array<string>) => Uint8Array)")]
  #[debug("MatchGroupTest(...)")]
  pub test: Option<BindingMatchGroupTest>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
  pub min_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
  pub max_size: Option<f64>,
  pub entries_aware: Option<bool>,
  pub entries_aware_merge_threshold: Option<f64>,
  pub tags: Option<Vec<String>>,
  pub include_dependencies_recursively: Option<bool>,
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
