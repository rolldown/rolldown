use std::any::type_name;

use futures::Future;
use napi::bindgen_prelude::{Either3, FromNapiValue};

use crate::types::js_hook::{JsHook, JsHookReturn};

pub trait JsHookExt<Args, Ret>: Send {
  fn call_async_normalized(
    &self,
    args: Args,
  ) -> impl Future<Output = Result<Ret, napi::Error>> + Send;
}

impl<Args: Send, Ret: FromNapiValue + Send> JsHookExt<Args, Ret> for JsHook<Args, Ret>
where
  JsHookReturn<Ret>: Send + FromNapiValue,
{
  /// Call the hook and normalize the returned `Either3<Promise<Ret>, Ret, UnknownReturnValue>` to `Result<Ret, napi::Error>`.
  #[allow(clippy::manual_async_fn)]
  fn call_async_normalized(
    &self,
    args: Args,
  ) -> impl Future<Output = Result<Ret, napi::Error>> + Send {
    async move {
      let ret = self.call_async(args).await?;
      match ret {
        Either3::A(p) => p.await,
        Either3::B(v) => Ok(v),
        Either3::C(_) => Err(napi::Error::new(
          napi::Status::InvalidArg,
          format!("Unknown return value. Cannot convert to `{}`.", type_name::<Ret>()),
        )),
      }
    }
  }
}
