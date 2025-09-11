use crate::types::binding_hmr_output::BindingHmrUpdate;
use crate::types::js_callback::JsCallback;
use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;

#[napi(object, object_to_js = false)]
pub struct BindingDevWatchOptions {
  pub skip_write: Option<bool>,
  pub use_polling: Option<bool>,
  pub poll_interval: Option<u32>,
  pub use_debounce: Option<bool>,
  pub debounce_duration: Option<u32>,
  pub compare_contents_for_polling: Option<bool>,
  pub debounce_tick_rate: Option<u32>,
}

#[napi(object, object_to_js = false)]
pub struct BindingDevOptions {
  #[napi(
    ts_type = "undefined | ((updates: BindingHmrUpdate[], changedFiles: string[]) => void | Promise<void>)"
  )]
  pub on_hmr_updates: Option<JsCallback<FnArgs<(Vec<BindingHmrUpdate>, Vec<String>)>, ()>>,
  pub watch: Option<BindingDevWatchOptions>,
}
