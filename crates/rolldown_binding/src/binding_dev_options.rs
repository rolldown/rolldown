use crate::options::BindingWatcherFileWatcherOptions;
use crate::types::binding_client_hmr_update::BindingClientHmrUpdate;
use crate::types::binding_outputs::BindingOutputs;
use crate::types::binding_rebuild_strategy::BindingRebuildStrategy;
use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};
use crate::types::error::BindingResult;
use crate::types::js_callback::JsCallback;
use napi::bindgen_prelude::FnArgs;

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingDevWatchOptions {
  pub enabled: Option<bool>,
  pub skip_write: Option<bool>,
  pub watcher: Option<BindingWatcherFileWatcherOptions>,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

impl From<BindingDevWatchOptions> for rolldown_dev::DevWatchOptions {
  fn from(value: BindingDevWatchOptions) -> Self {
    Self {
      enabled: value.enabled,
      skip_write: value.skip_write.unwrap_or_default(),
      watcher: value.watcher.map(Into::into),
      include: value.include.map(bindingify_string_or_regex_array),
      exclude: value.exclude.map(bindingify_string_or_regex_array),
    }
  }
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingDevOptions {
  #[napi(
    ts_type = "undefined | ((result: BindingResult<[BindingClientHmrUpdate[], string[]]>) => void | Promise<void>)"
  )]
  pub on_hmr_updates:
    Option<JsCallback<FnArgs<(BindingResult<(Vec<BindingClientHmrUpdate>, Vec<String>)>,)>, ()>>,
  #[napi(
    ts_type = "undefined | ((result: BindingResult<BindingOutputs>) => void | Promise<void>)"
  )]
  pub on_output: Option<JsCallback<FnArgs<(BindingResult<BindingOutputs>,)>, ()>>,
  /// Called with assets emitted while generating an HMR patch or compiling a
  /// lazy entry. These never go through `on_output`, so a consumer (e.g. Vite)
  /// must register this to serve them (e.g. write them to its in-memory files).
  #[napi(ts_type = "undefined | ((output: BindingOutputs) => void | Promise<void>)")]
  pub on_additional_assets: Option<JsCallback<FnArgs<(BindingOutputs,)>, ()>>,
  pub rebuild_strategy: Option<BindingRebuildStrategy>,
  pub watch: Option<BindingDevWatchOptions>,
  pub hot_update: Option<bool>,
}
