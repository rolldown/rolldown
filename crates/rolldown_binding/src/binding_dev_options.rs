use crate::types::binding_hmr_output::BindingHmrUpdate;
use crate::types::js_callback::JsCallback;
use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;

#[napi(object, object_to_js = false)]
pub struct BindingDevOptions {
  #[napi(ts_type = "undefined | ((updates: BindingHmrUpdate[]) => void | Promise<void>)")]
  pub on_hmr_updates: Option<JsCallback<FnArgs<(Vec<BindingHmrUpdate>,)>, ()>>,
}
