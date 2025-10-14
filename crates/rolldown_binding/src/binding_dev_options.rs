use crate::types::binding_client_hmr_update::BindingClientHmrUpdate;
use crate::types::binding_outputs::BindingOutputs;
use crate::types::binding_rebuild_strategy::BindingRebuildStrategy;
use crate::types::error::BindingResult;
use crate::types::js_callback::JsCallback;
use napi::bindgen_prelude::FnArgs;

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingDevWatchOptions {
  pub skip_write: Option<bool>,
  pub use_polling: Option<bool>,
  pub poll_interval: Option<u32>,
  pub use_debounce: Option<bool>,
  pub debounce_duration: Option<u32>,
  pub compare_contents_for_polling: Option<bool>,
  pub debounce_tick_rate: Option<u32>,
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
  pub rebuild_strategy: Option<BindingRebuildStrategy>,
  pub watch: Option<BindingDevWatchOptions>,
}
