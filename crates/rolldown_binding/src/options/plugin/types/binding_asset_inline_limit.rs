use std::sync::Arc;

use napi::{
  Either,
  bindgen_prelude::{Buffer, FnArgs, FromNapiValue},
  sys,
};
use rolldown_plugin_utils::UsizeOrFunction;

use crate::types::js_callback::{JsCallback, JsCallbackExt as _};

pub struct BindingAssetInlineLimit(UsizeOrFunction);

impl FromNapiValue for BindingAssetInlineLimit {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    unsafe {
      let value =
        Either::<u32, JsCallback<FnArgs<(String, Buffer)>, Option<bool>>>::from_napi_value(
          env, napi_val,
        )?;
      Ok(Self(match value {
        Either::A(value) => UsizeOrFunction::Number(value as usize),
        Either::B(func) => {
          UsizeOrFunction::Function(Arc::new(move |file: &str, content: &[u8]| {
            let file = file.to_string();
            let content = Buffer::from(content);
            let asset_inline_limit_fn = Arc::clone(&func);
            Box::pin(async move {
              asset_inline_limit_fn
                .invoke_async((file, content).into())
                .await
                .map_err(anyhow::Error::from)
            })
          }))
        }
      }))
    }
  }
}

impl From<BindingAssetInlineLimit> for UsizeOrFunction {
  fn from(value: BindingAssetInlineLimit) -> Self {
    value.0
  }
}
