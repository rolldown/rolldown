use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};
use crate::types::js_callback::JsCallback;
use derive_more::Debug;
use napi::{bindgen_prelude::FnArgs, threadsafe_function::ThreadsafeFunctionCallMode};
use rolldown::OnInvalidate;
use std::sync::Arc;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingWatchOption {
  pub skip_write: Option<bool>,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  pub build_delay: Option<u32>,
  #[napi(ts_type = "((id: string) => void) | undefined")]
  #[debug(skip)]
  pub on_invalidate: Option<JsCallback<FnArgs<(String,)>>>,
}

impl From<BindingWatchOption> for rolldown_common::WatchOption {
  fn from(value: BindingWatchOption) -> Self {
    Self {
      skip_write: value.skip_write.unwrap_or_default(),
      include: value.include.map(bindingify_string_or_regex_array),
      exclude: value.exclude.map(bindingify_string_or_regex_array),
      build_delay: value.build_delay,
      on_invalidate: value.on_invalidate.map(|js_callback| {
        OnInvalidate::new(Arc::new(move |path| {
          let f = Arc::clone(&js_callback);
          f.call(FnArgs { data: (path.to_string(),) }, ThreadsafeFunctionCallMode::Blocking);
        }))
      }),
    }
  }
}
