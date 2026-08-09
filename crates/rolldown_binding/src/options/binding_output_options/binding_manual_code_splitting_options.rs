use derive_more::Debug;
use napi::{Either, bindgen_prelude::FnArgs};
use rolldown::ChunkingContext;

use crate::types::{
  binding_module_info::BindingModuleInfo, binding_string_or_regex::BindingStringOrRegex,
  external_memory_status::ExternalMemoryStatus, js_callback::JsCallback,
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
  pub entries_aware: Option<bool>,
  pub entries_aware_merge_threshold: Option<f64>,
  pub tags: Option<Vec<String>>,
  pub include_dependencies_recursively: Option<bool>,
}

#[napi_derive::napi]
#[derive(Debug)]
pub struct BindingChunkingContext {
  inner: Option<ChunkingContext>,
}

impl BindingChunkingContext {
  pub fn new(inner: ChunkingContext) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&ChunkingContext> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed: this chunking context's native data was eagerly released after its group-name invocation settled. Use the context only while the callback runs.",
      )
    })
  }
}

#[napi_derive::napi]
impl BindingChunkingContext {
  // `ChunkingContext` keeps its `Arc` private, so unlike the other droppable
  // boxes no strong-count detail can be reported here.
  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self) -> ExternalMemoryStatus {
    match self.inner.take() {
      None => ExternalMemoryStatus {
        freed: false,
        reason: Some("Memory has already been freed".to_string()),
      },
      Some(_inner) => {
        // The `ChunkingContext` drops here automatically
        ExternalMemoryStatus { freed: true, reason: None }
      }
    }
  }

  #[napi]
  pub fn get_module_info(&self, module_id: String) -> napi::Result<Option<BindingModuleInfo>> {
    Ok(self.try_get_inner()?.get_module_info(&module_id).map(BindingModuleInfo::new))
  }
}
