//! JS functions of the bundler options, and their release at close.
//!
//! A [`JsCallback`] keeps a strong napi reference to its JS function, and that reference is a GC
//! root. JS can hold native objects (`BindingNormalizedOptions`, `BindingPluginContext`) that keep
//! the options alive. When a JS function of the options reaches one of those objects, the result
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
/// compiler stops a call of a function that the close of the owner cannot release.
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
/// gives an error. The exception is `call_and_forget`: after the release, it does nothing.
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
  /// function queues no call and returns `Status::Closing`.
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
    // Read the slot now. Then queue the call now, as `JsCallback::invoke_eager` does. The future
    // keeps the callback until the result arrives.
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
    // Take the callback out of the slot first. Then the callback drops after the lock guard drops.
    let released = self.write().unwrap_or_else(PoisonError::into_inner).take();
    drop(released);
  }
}

/// The group of JS functions of one set of bundler options.
///
/// `normalize_binding_options` creates one group for each call. The owner of the options gives it
/// to its [`SharedJsCallbackRelease`], which calls [`Self::release_all`].
///
/// The group keeps only weak references to the slots. So it does not keep a callback alive after
/// the options that use it are gone, for example when an option error stops a build before it
/// starts.
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

/// Call it while you hold no lock, because the release drops the JS functions.
fn release_groups(groups: Vec<JsCallbackRelease>) {
  for group in &groups {
    group.release_all();
  }
}

/// The release of all groups of one owner.
///
/// The owner shares it with the operations that its close does not wait for. Each such operation
/// takes a [`JsCallbackReleaseHold`] while it runs. These operations are:
///
/// - a build that started before `bundle.close()`,
/// - the close of a watcher event result,
/// - each dev engine `close()`.
///
/// The close of the owner requests the release at its end. The release runs when the last hold
/// drops. Only [`Self::request_release`] starts a release. A drop of the owner or of a holder never
/// starts it, because a GC can drop them while the owner still runs.
#[derive(Clone)]
pub struct SharedJsCallbackRelease {
  state: Arc<Mutex<SharedReleaseState>>,
}

struct SharedReleaseState {
  groups: Vec<JsCallbackRelease>,
  holds: usize,
  release_requested: bool,
}

impl SharedReleaseState {
  /// Takes the groups out when the release can run now.
  fn take_releasable(&mut self) -> Vec<JsCallbackRelease> {
    if self.release_requested && self.holds == 0 {
      std::mem::take(&mut self.groups)
    } else {
      Vec::new()
    }
  }
}

impl fmt::Debug for SharedJsCallbackRelease {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("SharedJsCallbackRelease")
  }
}

impl SharedJsCallbackRelease {
  pub fn new(groups: Vec<JsCallbackRelease>) -> Self {
    Self {
      state: Arc::new(Mutex::new(SharedReleaseState {
        groups,
        holds: 0,
        release_requested: false,
      })),
    }
  }

  /// Keeps the JS functions until the hold drops. Take the hold in the synchronous part of the
  /// operation, before it spawns its future, so that a release request that comes later waits for
  /// it. A hold that you take after the release does nothing.
  pub fn hold(&self) -> JsCallbackReleaseHold {
    self.lock().holds += 1;
    JsCallbackReleaseHold(self.clone())
  }

  /// Adds the group of a new operation and takes a hold for it, under one lock. With two separate
  /// steps, a release on another thread can release the new group before the hold exists. If the
  /// release request came first, the drop of the last hold releases the group.
  pub fn hold_with_group(&self, group: JsCallbackRelease) -> JsCallbackReleaseHold {
    let mut state = self.lock();
    state.groups.push(group);
    state.holds += 1;
    drop(state);
    JsCallbackReleaseHold(self.clone())
  }

  /// Releases the JS functions now, or when the last hold drops.
  pub fn request_release(&self) {
    let groups = {
      let mut state = self.lock();
      state.release_requested = true;
      state.take_releasable()
    };
    release_groups(groups);
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, SharedReleaseState> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

/// See [`SharedJsCallbackRelease::hold`].
pub struct JsCallbackReleaseHold(SharedJsCallbackRelease);

impl Drop for JsCallbackReleaseHold {
  fn drop(&mut self) {
    let groups = {
      let mut state = self.0.lock();
      state.holds -= 1;
      state.take_releasable()
    };
    release_groups(groups);
  }
}

/// Requests the release of a [`SharedJsCallbackRelease`] when it drops. Keep it as the first local
/// of the close future of the owner, so that the request runs on each exit path.
pub struct RequestReleaseOnDrop(pub SharedJsCallbackRelease);

impl Drop for RequestReleaseOnDrop {
  fn drop(&mut self) {
    self.0.request_release();
  }
}
