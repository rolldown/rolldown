use crate::options::BindingWatcherFileWatcherOptions;
use crate::types::binding_hmr_update::BindingHmrUpdate;
use crate::types::binding_outputs::BindingOutputs;
use crate::types::binding_rebuild_strategy::BindingRebuildStrategy;
use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};
use crate::types::error::BindingResult;
use crate::types::js_callback::{JsCallback, MaybeAsyncJsCallback};
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
  /// When the function returns a `Promise`, a rebuild that follows the update in the same task
  /// waits for it.
  #[napi(
    ts_type = "undefined | ((result: BindingResult<[BindingHmrUpdate, string[]]>) => void | Promise<void>)"
  )]
  pub on_hmr_updates:
    Option<MaybeAsyncJsCallback<FnArgs<(BindingResult<(BindingHmrUpdate, Vec<String>)>,)>, ()>>,
  /// `hmrGeneration` is the HMR generation the output is rendered at, when the build succeeded.
  /// The output code does not carry it, so the consumer passes it to the page that loads this
  /// output (the runtime's `setOutputGeneration`).
  #[napi(
    ts_type = "undefined | ((result: BindingResult<BindingOutputs>, hmrGeneration: number | undefined) => void | Promise<void>)"
  )]
  pub on_output: Option<JsCallback<FnArgs<(BindingResult<BindingOutputs>, Option<u32>)>, ()>>,
  /// Called with assets emitted while generating an HMR patch or compiling a
  /// lazy entry. These never go through `on_output`, so a consumer (e.g. Vite)
  /// must register this to serve them (e.g. write them to its in-memory files).
  #[napi(ts_type = "undefined | ((output: BindingOutputs) => void | Promise<void>)")]
  pub on_additional_assets: Option<JsCallback<FnArgs<(BindingOutputs,)>, ()>>,
  pub rebuild_strategy: Option<BindingRebuildStrategy>,
  pub watch: Option<BindingDevWatchOptions>,
  pub hot_update: Option<bool>,
}
