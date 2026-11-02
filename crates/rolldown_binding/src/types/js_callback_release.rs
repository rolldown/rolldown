//! JS functions of the bundler options, and their release at close.
//!
//! A [`JsCallback`] keeps a strong napi reference to its JS function, and that reference is a GC
//! root. JS can hold native objects (`BindingNormalizedOptions`, `BindingPluginContext`) that keep
//! the options alive. When a JS function of the options reaches one of those JS objects, the result
//! is a cycle that the GC cannot free. So the owner of the options releases all of their JS
//! functions when it closes.
//! See internal-docs/bundler-data-lifecycle/implementation.md ("JS callback lifetime").

use std::{
  fmt,
  sync::{Arc, Mutex, PoisonError, RwLock, Weak},
};

use futures::Future;
use napi::{
  Either, Status, ValueType,
  bindgen_prelude::{FromNapiValue, JsValuesTupleIntoVec, Promise, TypeName, ValidateNapiValue},
  sys,
  threadsafe_function::ThreadsafeFunctionCallMode,
};

use super::js_callback::{
  InvalidReturnValue, JsCallback, JsCallbackExt, MaybeAsyncJsCallbackExt, invoke_eager_on,
};

/// A JS function of the bundler options, as napi gives it to Rust.
///
/// This type has no call methods. Give it to [`JsCallbackRelease::track`] to get a
/// [`ReleasableJsCallback`] that you can call. Use this type, not [`JsCallback`], for each function
/// field of the bundler options (the input and output options of `BindingBundlerOptions`). Then the
/// compiler stops a call of a function that the close of the bundle cannot release.
pub type UntrackedJsCallback<Args = (), Ret = ()> = Untracked<JsCallback<Args, Ret>>;

/// Shortcut for an [`UntrackedJsCallback`] whose JS function can return a `Promise`.
pub type UntrackedMaybeAsyncJsCallback<Args = (), Ret = ()> =
  UntrackedJsCallback<Args, Either<Promise<Ret>, Ret>>;

/// A JS function of the bundler options that [`JsCallbackRelease::release_all`] can release.
pub type ReleasableJsCallback<Args = (), Ret = ()> = Releasable<JsCallback<Args, Ret>>;

/// See [`UntrackedJsCallback`].
pub struct Untracked<T>(T);

impl<T> fmt::Debug for Untracked<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("UntrackedJsCallback")
  }
}

impl<T: TypeName> TypeName for Untracked<T> {
  fn type_name() -> &'static str {
    T::type_name()
  }

  fn value_type() -> ValueType {
    T::value_type()
  }
}

impl<T: ValidateNapiValue> ValidateNapiValue for Untracked<T> {
  unsafe fn validate(
    env: sys::napi_env,
    napi_val: sys::napi_value,
  ) -> napi::Result<sys::napi_value> {
    unsafe { T::validate(env, napi_val) }
  }
}

impl<T: FromNapiValue> FromNapiValue for Untracked<T> {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    unsafe { T::from_napi_value(env, napi_val) }.map(Self)
  }
}

/// See [`ReleasableJsCallback`].
///
/// Each call reads the slot one time, at call time, and keeps its own clone of the callback until
/// the call ends. So a call that starts before the release ends normally. A call after the release
/// gives an error, except `call_and_forget`, which then does nothing.
pub struct Releasable<T> {
  slot: Arc<RwLock<Option<T>>>,
}

impl<T> Clone for Releasable<T> {
  fn clone(&self) -> Self {
    Self { slot: Arc::clone(&self.slot) }
  }
}

impl<T> fmt::Debug for Releasable<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("ReleasableJsCallback")
  }
}

impl<T: Clone> Releasable<T> {
  fn get(&self) -> Option<T> {
    self.slot.read().unwrap_or_else(PoisonError::into_inner).clone()
  }

  fn get_or_err(&self) -> napi::Result<T> {
    self.get().ok_or_else(|| {
      napi::Error::new(
        Status::Closing,
        "A JS function of the bundler options was called after the bundle closed. \
         The close releases these functions.",
      )
    })
  }
}

impl<Args, Ret> ReleasableJsCallback<Args, Ret>
where
  Args: 'static + JsValuesTupleIntoVec,
  Ret: 'static,
  Either<Ret, InvalidReturnValue>: 'static + FromNapiValue,
{
  /// Queues a call of the JS function and does not wait for its result. After the release, this
  /// does nothing and returns `Status::Closing`.
  pub fn call_and_forget(&self, args: Args, mode: ThreadsafeFunctionCallMode) -> Status {
    match self.get() {
      Some(callback) => callback.call(args, mode),
      None => Status::Closing,
    }
  }
}

impl<Args, Ret> JsCallbackExt<Args, Ret> for ReleasableJsCallback<Args, Ret>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue + TypeName,
  Either<Ret, InvalidReturnValue>: FromNapiValue,
{
  async fn invoke_async(&self, args: Args) -> Result<Ret, napi::Error> {
    let callback = self.get_or_err()?;
    callback.invoke_async(args).await
  }

  fn invoke_eager(
    &self,
    args: Args,
  ) -> impl Future<Output = Result<Ret, napi::Error>> + Send + 'static {
    // Read the slot and queue the call now, as `JsCallback::invoke_eager` does. The future keeps
    // the callback until the result arrives.
    let call = self.get_or_err().map(|callback| {
      let result = invoke_eager_on(&callback, args);
      (callback, result)
    });
    async move {
      let (_callback, result) = call?;
      result.await
    }
  }
}

impl<Args, Ret> MaybeAsyncJsCallbackExt<Args, Ret>
  for ReleasableJsCallback<Args, Either<Promise<Ret>, Ret>>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue + TypeName,
  Either<Either<Promise<Ret>, Ret>, InvalidReturnValue>: FromNapiValue,
{
  async fn await_call(&self, args: Args) -> Result<Ret, napi::Error> {
    let callback = self.get_or_err()?;
    callback.await_call(args).await
  }
}

trait ReleaseSlot: Send + Sync {
  fn release(&self);
}

impl<T: Send + Sync> ReleaseSlot for RwLock<Option<T>> {
  fn release(&self) {
    // Take the callback out first. Its drop then runs without the lock.
    let released = self.write().unwrap_or_else(PoisonError::into_inner).take();
    drop(released);
  }
}

/// The group of JS functions of one set of bundler options.
///
/// `normalize_binding_options` creates one group for each call, and the owner of the options keeps
/// it. The owner calls [`Self::release_all`] one time, at the end of its close operation.
///
/// The group keeps only weak references to the slots. So it does not keep a callback alive after
/// the options that use it are gone, for example when a build stops before it starts.
#[derive(Clone, Default)]
pub struct JsCallbackRelease {
  slots: Arc<Mutex<Vec<Weak<dyn ReleaseSlot>>>>,
}

impl fmt::Debug for JsCallbackRelease {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("JsCallbackRelease")
  }
}

impl JsCallbackRelease {
  pub fn track<T: Send + Sync + 'static>(&self, callback: Untracked<T>) -> Releasable<T> {
    let slot = Arc::new(RwLock::new(Some(callback.0)));
    let release_slot: Arc<dyn ReleaseSlot> = Arc::clone(&slot) as Arc<dyn ReleaseSlot>;
    self.slots.lock().unwrap_or_else(PoisonError::into_inner).push(Arc::downgrade(&release_slot));
    Releasable { slot }
  }

  /// Releases each JS function of the group. A second call does nothing.
  pub fn release_all(&self) {
    let slots = std::mem::take(&mut *self.slots.lock().unwrap_or_else(PoisonError::into_inner));
    for slot in slots.iter().filter_map(Weak::upgrade) {
      slot.release();
    }
  }
}

/// Releases its groups when it drops. Keep it as the first local of a close future, so that the
/// release runs on each exit path, also after an error.
pub struct ReleaseJsCallbacksOnDrop(pub Vec<JsCallbackRelease>);

impl Drop for ReleaseJsCallbacksOnDrop {
  fn drop(&mut self) {
    for group in &self.0 {
      group.release_all();
    }
  }
}
