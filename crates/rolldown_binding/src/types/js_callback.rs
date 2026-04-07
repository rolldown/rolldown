use std::sync::Arc;

use futures::Future;
use napi::{
  Either, Status,
  bindgen_prelude::{FromNapiValue, JsValuesTupleIntoVec, Promise, TypeName, ValidateNapiValue},
  threadsafe_function::ThreadsafeFunction,
};
use rolldown_utils::debug::pretty_type_name;

/// A catch-all fallback type for `Either<Ret, AnyReturnValue>` that accepts any JavaScript value.
///
/// Unlike NAPI-RS's [`napi::threadsafe_function::UnknownReturnValue`], which has
/// `value_type() = ValueType::Unknown` (a value that never matches any real JS type), this
/// type's `validate` always returns `Ok`. This is important because the `FromNapiValue` impl for
/// `Either<A, B>` calls `validate` on each variant in order. If both fail, NAPI-RS calls
/// `napi_fatal_exception`, causing an unhandled JS exception rather than returning an `Err`
/// through the Rust error channel.
///
/// By using `AnyReturnValue` as the fallback in `Either<Ret, AnyReturnValue>`, invalid return
/// values from JS callbacks are captured as `Either::B(AnyReturnValue)` and flow through the
/// Rust error path via [`create_unknown_return_error`], avoiding fatal exceptions.
pub struct AnyReturnValue;

impl TypeName for AnyReturnValue {
  fn type_name() -> &'static str {
    "AnyReturnValue"
  }

  fn value_type() -> napi::ValueType {
    napi::ValueType::Unknown
  }
}

impl ValidateNapiValue for AnyReturnValue {
  unsafe fn validate(
    _env: napi::sys::napi_env,
    _napi_val: napi::sys::napi_value,
  ) -> napi::Result<napi::sys::napi_value> {
    // Always valid — this is the catch-all fallback in Either<Ret, AnyReturnValue>
    Ok(std::ptr::null_mut())
  }
}

impl FromNapiValue for AnyReturnValue {
  unsafe fn from_napi_value(
    _env: napi::sys::napi_env,
    _napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    Ok(AnyReturnValue)
  }
}

/// `JsCallback`  is a type alias for `ThreadsafeFunction`. It represents a JavaScript function that passed to Rust side.
/// Related concepts are complex, so we use `JsCallback` to simplify the mental model. For details, please refer to:
/// - https://napi.rs/docs/compat-mode/concepts/thread-safe-function.en
/// - https://github.com/napi-rs/napi-rs
///
/// ## Examples
///
/// You will notice that I place Js under the Rust type, because the Js type is generated to fit the Rust type.
/// We need write correct Js types to satisfy the Rust type not the other way around.
///
/// ### Sync
///
/// - Rust: `JsCallback<FnArgs<(String, i32)>, i32>`
/// - Js: `(a: string, b: number) => number`
///
/// For `Option<T>` in params position, when you pass `None` in Rust, it will be `null` in Js. However, NAPI-RS generates
/// `undefined | null | T` as the Js type. So, we introduce `Nullable<T>` in Js to simplify the writing.
///
/// One tricky part for `Option<T>` in return position is that the corresponding Js type is `T | null | undefined` will make
/// Ts compiler force you to write `return` statement explicitly. To avoid this, we introduce `VoidNullable<T>` in Js. It will
/// expand to `T | null | undefined | void`.
///
/// - Rust: `JsCallback<FnArgs<(Option<String>, i32)>, Option<i32>>`
/// - Js: `(a: string | null | undefined, b: number) => number | null | undefined | void`
/// - Js(Simplified): `(a: Nullable<string>, b: number) => VoidNullable<number>`
///
/// ### Async
///
/// For async functions in Js, remember these functions are also sync functions that return a `Promise<T>`. What's good is that
/// Ts compiler force you to add `Promise<T>` in the return type when you write async functions. So, you could consider they are
/// just sync functions that return `Promise<T>`.
///
/// - Rust: `JsCallback<FnArgs<(String, i32)>, Promise<i32>>`
/// - Js: `(a: string, b: number) => Promise<number>`
///
/// ---
///
/// - Rust: `JsCallback<FnArgs<(Option<String>, i32)>, Promise<Option<i32>>>`
/// - Js: `(a: string | null | undefined, b: number) => Promise<number | null | undefined | void>`
/// - Js(Simplified): `(a: Nullable<string>, b: number) => Promise<VoidNullable<number>>`
///
/// ### MaybeAsync
///
/// Sometimes we want to accept functions that could be sync or async, as we just said above, it's actually just a sync function
/// that returns `Promise<T> | T`. This pattern is very common in NAPI-RS, so we introduce `MaybeAsyncJsCallback` to simplify it.
///
/// Notice the order matters for rust types `Either<Promise<T>, T>` and `Either<T, Promise<T>>`. Always use `Either<Promise<T>, T>`.
///
/// - Rust: `JsCallback<FnArgs<(String, i32), Either<Promise<i32>>, i32>>`
/// - Rust(Simplified): `MaybeAsyncJsCallback<FnArgs<(String, i32)>, i32>`
/// - Js: `(a: string, b: number) => Promise<number> | number`
/// - Js(Simplified): `(a: string, b: number) => MaybePromise<number>`
///
/// ---
///
/// - Rust: `JsCallback<FnArgs<(Option<String>, i32), Either<Promise<Option<i32>>>, Option<i32>>`
/// - Rust(Simplified): `MaybeAsyncJsCallback<FnArgs<(Option<String>, i32)>, Option<i32>>`
/// - Js: `(a: string | null | undefined, b: number) => Promise<number | null | undefined | void> | number | null | undefined | void`
/// - Js(Simplified): `(a: Nullable<string>, b: number) => MaybePromise<VoidNullable<number>>`
pub type JsCallback<Args = (), Ret = ()> =
  Arc<ThreadsafeFunction<Args, Either<Ret, AnyReturnValue>, Args, Status, false, true>>;

/// Shortcut for `JsCallback<FnArgs<..., Either<Promise<Ret>, Ret>>`, which could be simplified to `MaybeAsyncJsCallback<...>, Ret>`.
pub type MaybeAsyncJsCallback<Args = (), Ret = ()> = JsCallback<Args, Either<Promise<Ret>, Ret>>;

pub trait JsCallbackExt<Args, Ret> {
  fn invoke_async(&self, args: Args) -> impl Future<Output = Result<Ret, napi::Error>> + Send;
}

impl<Args, Ret> JsCallbackExt<Args, Ret> for JsCallback<Args, Ret>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue,
  napi::Either<Ret, AnyReturnValue>: FromNapiValue,
{
  async fn invoke_async(&self, args: Args) -> Result<Ret, napi::Error> {
    match self.call_async(args).await? {
      Either::A(ret) => Ok(ret),
      Either::B(_unknown) => create_unknown_return_error::<Ret, Self>(),
    }
  }
}

fn create_unknown_return_error<Ret, T>() -> Result<Ret, napi::Error> {
  // TODO: should provide more information about the unknown return value
  let js_type = "unknown";
  let expected_rust_type = pretty_type_name::<Ret>();

  Err(napi::Error::new(
    napi::Status::InvalidArg,
    format!(
      "UNKNOWN_RETURN_VALUE. Cannot convert {js_type} to `{expected_rust_type}` in {}.",
      pretty_type_name::<T>(),
    ),
  ))
}

pub trait MaybeAsyncJsCallbackExt<Args, Ret> {
  /// Call Js function asynchronously in rust. If the Js function returns `Promise<T>`, it will unwrap/await the promise and return `T`.
  fn await_call(&self, args: Args) -> impl Future<Output = Result<Ret, napi::Error>> + Send;
}

impl<Args, Ret> MaybeAsyncJsCallbackExt<Args, Ret> for JsCallback<Args, Either<Promise<Ret>, Ret>>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue,
  napi::Either<napi::Either<Promise<Ret>, Ret>, AnyReturnValue>: FromNapiValue,
{
  #[expect(clippy::manual_async_fn)]
  fn await_call(&self, args: Args) -> impl Future<Output = Result<Ret, napi::Error>> + Send {
    async move {
      match self.call_async(args).await? {
        Either::A(Either::A(promise)) => promise.await,
        Either::A(Either::B(ret)) => Ok(ret),
        Either::B(_unknown) => {
          // TODO: should provide more information about the unknown return value
          let js_type = "unknown";
          let expected_rust_type = pretty_type_name::<Ret>();

          Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
              "UNKNOWN_RETURN_VALUE. Cannot convert {js_type} to `{expected_rust_type}` in {}.",
              pretty_type_name::<Self>(),
            ),
          ))
        }
      }
    }
  }
}
