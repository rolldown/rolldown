use std::{fmt::Debug, marker::PhantomData};

use napi::{
  bindgen_prelude::{FromNapiValue, Promise, ValidateNapiValue},
  threadsafe_function::{ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction},
  Either, JsFunction,
};

use super::IntoJsUnknownVec;
use crate::NAPI_ENV;

pub trait JsCallbackArgs: IntoJsUnknownVec + Send + Sync + 'static {}
impl<T: IntoJsUnknownVec + Send + Sync + 'static> JsCallbackArgs for T {}
pub trait JsCallbackRet: FromNapiValue + ValidateNapiValue + Send + 'static {}
impl<T: FromNapiValue + ValidateNapiValue + Send + 'static> JsCallbackRet for T {}

pub struct JsCallback<Args: JsCallbackArgs, Ret: JsCallbackRet> {
  _args: PhantomData<Args>,
  _ret: PhantomData<Ret>,
  ts_fn: ThreadsafeFunction<Args, ErrorStrategy::Fatal>,
}

impl<Args: JsCallbackArgs + Debug, Ret: JsCallbackRet> JsCallback<Args, Ret> {
  pub fn new(js_fn: &JsFunction) -> napi::Result<Self> {
    let mut ts_fn: ThreadsafeFunction<Args, ErrorStrategy::Fatal> = js_fn
      .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<Args>| {
        ctx.value.into_js_unknown_vec(&ctx.env)
      })?;
    NAPI_ENV.with(|env| ts_fn.unref(env))?;
    Ok(Self {
      _args: PhantomData,
      _ret: PhantomData,
      ts_fn,
    })
  }

  /// This method is already handle case return Promise<Ret>
  pub(crate) async fn call_async(&self, args: Args) -> napi::Result<Ret> {
    let ret: Either<Ret, Promise<Ret>> = self.ts_fn.call_async(args).await?;

    match ret {
      Either::A(ret) => Ok(ret),
      Either::B(promise) => promise.await,
    }
  }
}

impl<Args: JsCallbackArgs, Ret: JsCallbackRet> Clone for JsCallback<Args, Ret> {
  fn clone(&self) -> Self {
    Self {
      _args: PhantomData,
      _ret: PhantomData,
      ts_fn: self.ts_fn.clone(),
    }
  }
}
