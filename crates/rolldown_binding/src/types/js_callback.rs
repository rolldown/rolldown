use std::sync::{Arc, Condvar, Mutex};

use futures::Future;
use napi::{
  Either, Status,
  bindgen_prelude::{FromNapiValue, JsValuesTupleIntoVec, Promise},
  threadsafe_function::{ThreadsafeFunction, UnknownReturnValue},
};
use rolldown_error::{BuildDiagnostic, SingleBuildResult};
use rolldown_utils::debug::pretty_type_name;

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
  Arc<ThreadsafeFunction<Args, Either<Ret, UnknownReturnValue>, Args, Status, false, true>>;

/// Shortcut for `JsCallback<FnArgs<..., Either<Promise<Ret>, Ret>>`, which could be simplified to `MaybeAsyncJsCallback<...>, Ret>`.
pub type MaybeAsyncJsCallback<Args = (), Ret = ()> = JsCallback<Args, Either<Promise<Ret>, Ret>>;

pub trait JsCallbackExt<Args, Ret> {
  fn invoke_async(&self, args: Args) -> impl Future<Output = SingleBuildResult<Ret>> + Send;
  fn invoke_sync(&self, args: Args) -> SingleBuildResult<Ret>;
}

impl<Args, Ret> JsCallbackExt<Args, Ret> for JsCallback<Args, Ret>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue,
  napi::Either<Ret, UnknownReturnValue>: FromNapiValue,
{
  async fn invoke_async(&self, args: Args) -> SingleBuildResult<Ret> {
    match self.call_async(args).await? {
      Either::A(ret) => Ok(ret),
      Either::B(_unknown) => create_unknown_return_error::<Ret, Self>(),
    }
  }

  fn invoke_sync(&self, args: Args) -> SingleBuildResult<Ret> {
    let init_value = Ok(Either::B(UnknownReturnValue));
    let pair = Arc::new((Mutex::new(init_value), Condvar::new()));
    let pair_clone = Arc::clone(&pair);

    self.call_with_return_value(
      args,
      napi::threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
      move |ret, _env| {
        let (lock, cvar) = &*pair;
        *lock.lock().unwrap() = ret;
        cvar.notify_one();
        Ok(())
      },
    );

    let (lock, cvar) = &*pair_clone;
    let notified = lock.lock().unwrap();
    let mut res = cvar.wait(notified).map_err(|err| {
      BuildDiagnostic::napi_error(napi::Error::new(
        napi::Status::GenericFailure,
        format!("PoisonError: {err:?}",),
      ))
    })?;
    let res = res.as_mut().map_err(|err| {
      BuildDiagnostic::napi_error(napi::Error::new(
        napi::Status::GenericFailure,
        format!("{err:?}",),
      ))
    })?;

    match std::mem::replace(res, Either::B(UnknownReturnValue)) {
      Either::A(ret) => Ok(ret),
      Either::B(_unknown) => create_unknown_return_error::<Ret, Self>(),
    }
  }
}

fn create_unknown_return_error<Ret, T>() -> SingleBuildResult<Ret> {
  // TODO: should provide more information about the unknown return value
  let js_type = "unknown";
  let expected_rust_type = pretty_type_name::<Ret>();

  Err(BuildDiagnostic::napi_error(napi::Error::new(
    napi::Status::InvalidArg,
    format!(
      "UNKNOWN_RETURN_VALUE. Cannot convert {js_type} to `{expected_rust_type}` in {}.",
      pretty_type_name::<T>(),
    ),
  )))
}

pub trait MaybeAsyncJsCallbackExt<Args, Ret> {
  /// Call Js function asynchronously in rust. If the Js function returns `Promise<T>`, it will unwrap/await the promise and return `T`.
  fn await_call(&self, args: Args) -> impl Future<Output = SingleBuildResult<Ret>> + Send;
}

impl<Args, Ret> MaybeAsyncJsCallbackExt<Args, Ret> for JsCallback<Args, Either<Promise<Ret>, Ret>>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue,
  napi::Either<napi::Either<Promise<Ret>, Ret>, UnknownReturnValue>: FromNapiValue,
{
  #[expect(clippy::manual_async_fn)]
  fn await_call(&self, args: Args) -> impl Future<Output = SingleBuildResult<Ret>> + Send {
    async move {
      match self.call_async(args).await? {
        Either::A(Either::A(promise)) => Ok(promise.await?),
        Either::A(Either::B(ret)) => Ok(ret),
        Either::B(_unknown) => {
          // TODO: should provide more information about the unknown return value
          let js_type = "unknown";
          let expected_rust_type = pretty_type_name::<Ret>();

          let napi_error = napi::Error::new(
            napi::Status::InvalidArg,
            format!(
              "UNKNOWN_RETURN_VALUE. Cannot convert {js_type} to `{expected_rust_type}` in {}.",
              pretty_type_name::<Self>(),
            ),
          );

          Err(BuildDiagnostic::napi_error(napi_error))
        }
      }
    }
  }
}
