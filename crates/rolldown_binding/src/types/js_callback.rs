use std::{ptr, sync::Arc};

use futures::Future;
use napi::{
  Either, Status, ValueType,
  bindgen_prelude::{FromNapiValue, JsValuesTupleIntoVec, Promise, TypeName, ValidateNapiValue},
  sys,
  threadsafe_function::ThreadsafeFunction,
};

/// Used as the fallback branch in `Either<Ret, InvalidReturnValue>` to catch
/// type mismatches from JS function options. Always passes NAPI validation so
/// that when `Ret` validation fails, the error is handled in Rust with a clear
/// message instead of becoming an uncatchable `napi_fatal_exception`.
pub struct InvalidReturnValue {
  pub value_type: ValueType,
}

impl TypeName for InvalidReturnValue {
  fn type_name() -> &'static str {
    "InvalidReturnValue"
  }

  fn value_type() -> ValueType {
    ValueType::Unknown
  }
}

impl ValidateNapiValue for InvalidReturnValue {
  unsafe fn validate(
    _env: sys::napi_env,
    _napi_val: sys::napi_value,
  ) -> napi::Result<sys::napi_value> {
    Ok(ptr::null_mut())
  }
}

impl FromNapiValue for InvalidReturnValue {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    let value_type = napi::type_of!(env, napi_val)?;
    Ok(InvalidReturnValue { value_type })
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
  Arc<ThreadsafeFunction<Args, Either<Ret, InvalidReturnValue>, Args, Status, false, true>>;

/// Shortcut for `JsCallback<FnArgs<..., Either<Promise<Ret>, Ret>>`, which could be simplified to `MaybeAsyncJsCallback<...>, Ret>`.
pub type MaybeAsyncJsCallback<Args = (), Ret = ()> = JsCallback<Args, Either<Promise<Ret>, Ret>>;

pub trait JsCallbackExt<Args, Ret> {
  fn invoke_async(&self, args: Args) -> impl Future<Output = Result<Ret, napi::Error>> + Send;
}

impl<Args, Ret> JsCallbackExt<Args, Ret> for JsCallback<Args, Ret>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue + TypeName,
  napi::Either<Ret, InvalidReturnValue>: FromNapiValue,
{
  async fn invoke_async(&self, args: Args) -> Result<Ret, napi::Error> {
    match self.call_async_catch(args).await? {
      Either::A(ret) => Ok(ret),
      Either::B(invalid) => Err(create_invalid_return_error(
        invalid.value_type,
        Ret::value_type(),
        rolldown_utils::debug::pretty_type_name::<Self>(),
      )),
    }
  }
}

fn js_type_name(value_type: ValueType) -> &'static str {
  match value_type {
    ValueType::Undefined => "undefined",
    ValueType::Null => "null",
    ValueType::Boolean => "boolean",
    ValueType::Number => "number",
    ValueType::String => "string",
    ValueType::Symbol => "symbol",
    ValueType::Object => "object",
    ValueType::Function => "function",
    ValueType::External => "external",
    ValueType::Unknown => "unknown",
  }
}

fn create_invalid_return_error(
  received: ValueType,
  expected: ValueType,
  fn_type: std::borrow::Cow<'_, str>,
) -> napi::Error {
  napi::Error::new(
    Status::InvalidArg,
    format!(
      "The function returned `{}`, but expected `{}` in `{fn_type}`.",
      js_type_name(received),
      js_type_name(expected),
    ),
  )
}

pub trait MaybeAsyncJsCallbackExt<Args, Ret> {
  /// Call Js function asynchronously in rust. If the Js function returns `Promise<T>`, it will unwrap/await the promise and return `T`.
  fn await_call(&self, args: Args) -> impl Future<Output = Result<Ret, napi::Error>> + Send;
}

impl<Args, Ret> MaybeAsyncJsCallbackExt<Args, Ret> for JsCallback<Args, Either<Promise<Ret>, Ret>>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue + TypeName,
  napi::Either<napi::Either<Promise<Ret>, Ret>, InvalidReturnValue>: FromNapiValue,
{
  async fn await_call(&self, args: Args) -> Result<Ret, napi::Error> {
    match self.call_async_catch(args).await? {
      Either::A(Either::A(promise)) => promise.await,
      Either::A(Either::B(ret)) => Ok(ret),
      Either::B(invalid) => Err(create_invalid_return_error(
        invalid.value_type,
        Ret::value_type(),
        rolldown_utils::debug::pretty_type_name::<Self>(),
      )),
    }
  }
}
