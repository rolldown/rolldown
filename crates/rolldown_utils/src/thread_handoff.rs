//! Wasm only: run a hook wherever scheduled work starts or resumes on a thread.
//!
//! Threaded WASI needs this for a V8 bug: a thread keeps a stale view of the shared memory
//! size after another thread grows it, and `memory.fill` / `memory.copy` / atomics trap
//! against that view. `rolldown_binding` refreshes the view after every allocation, which
//! covers blocks the thread allocates itself. A task that allocated on worker A can yield
//! and resume on worker B, and a blocking closure built on one thread runs on another; B
//! then fills or copies into existing capacity without allocating, so no refresh runs on B.
//! This module closes that part: every future and blocking closure that enters the shared
//! scheduler through `rolldown_utils::async_runtime` calls the hook at the start of each
//! `poll` and at the start of the closure body. The binding registers its refresh here at
//! module init. See internal-docs/wasi-shared-memory-grow/design.md
//!
//! The hook is set only on the threaded WASI build. On threadless wasm it is never set, so
//! each call is one load of an unset `OnceLock`. Native builds do not compile this module:
//! `async_runtime` there is the plain re-export of `napi_async_runtime`.
//!
//! `try_spawn_blocking` is not wrapped here: its rejection hands the caller's closure back,
//! and a wrapped closure cannot be unwrapped. Its only in-tree caller, the napi adapter in
//! `rolldown_binding`, boxes its closure with the hook itself.
use std::{
  future::Future,
  pin::{Pin, pin},
  sync::OnceLock,
  task::{Context, Poll},
};

use napi_async_runtime::{JoinHandle, RuntimeConfigError};

static HOOK: OnceLock<fn()> = OnceLock::new();

/// Install the hook run at every scheduler handoff. The first call wins; later calls are
/// ignored (module init can run once per Node environment, always with the same hook).
pub fn set_thread_handoff_hook(hook: fn()) {
  let _ = HOOK.set(hook);
}

/// Run the installed hook, if any. Call it where work arrives on a thread from elsewhere.
#[inline]
pub fn on_thread_handoff() {
  if let Some(hook) = HOOK.get() {
    hook();
  }
}

/// A future that runs [`on_thread_handoff`] before every poll of the inner future.
pub struct HandoffHook<F> {
  inner: F,
}

impl<F> HandoffHook<F> {
  #[inline]
  pub const fn new(inner: F) -> Self {
    Self { inner }
  }

  #[inline]
  pub fn into_inner(self) -> F {
    self.inner
  }
}

impl<F: Future> Future for HandoffHook<F> {
  type Output = F::Output;

  #[inline]
  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
    on_thread_handoff();
    // SAFETY: structural pinning of the only field. `inner` is never moved out of a pinned
    // `HandoffHook` (`into_inner` takes `self` by value), there is no `Drop` impl, and the
    // auto `Unpin` impl holds only when `F: Unpin`.
    unsafe { self.map_unchecked_mut(|this| &mut this.inner) }.poll(cx)
  }
}

// The entry points below shadow the glob re-export in `crate::async_runtime` on wasm. They
// keep the runtime's signatures, so every caller (the facades in `crate::futures`, the
// module loader, the napi adapter) goes through them unchanged.

#[inline]
pub fn spawn<F, T>(future: F) -> JoinHandle<T>
where
  F: Future<Output = T> + Send + 'static,
  T: Send + 'static,
{
  napi_async_runtime::spawn(HandoffHook::new(future))
}

#[inline]
pub fn try_spawn<F, T>(future: F) -> Result<JoinHandle<T>, (RuntimeConfigError, F)>
where
  F: Future<Output = T> + Send + 'static,
  T: Send + 'static,
{
  napi_async_runtime::try_spawn(HandoffHook::new(future))
    .map_err(|(error, future)| (error, future.into_inner()))
}

#[inline]
pub fn try_spawn_detached<F>(future: F) -> Result<(), F>
where
  F: Future<Output = ()> + Send + 'static,
{
  napi_async_runtime::try_spawn_detached(HandoffHook::new(future)).map_err(HandoffHook::into_inner)
}

#[inline]
pub fn spawn_detached<F>(future: F)
where
  F: Future<Output = ()> + Send + 'static,
{
  napi_async_runtime::spawn_detached(HandoffHook::new(future));
}

#[inline]
pub fn spawn_blocking<F, T>(function: F) -> JoinHandle<T>
where
  F: FnOnce() -> T + Send + 'static,
  T: Send + 'static,
{
  napi_async_runtime::spawn_blocking(move || {
    on_thread_handoff();
    function()
  })
}

/// `block_on` polls on the calling thread, but it parks between polls while other threads
/// grow the memory and send it blocks, so each poll starts with the hook too.
#[inline]
pub fn block_on<F: Future>(future: F) -> F::Output {
  napi_async_runtime::block_on(HandoffHook::new(future))
}

#[inline]
pub fn block_on_dyn(future: Pin<&mut dyn Future<Output = ()>>) {
  napi_async_runtime::block_on_dyn(pin!(HandoffHook::new(future)));
}

#[inline]
pub fn try_block_on_dyn(
  future: Pin<&mut dyn Future<Output = ()>>,
) -> Result<(), RuntimeConfigError> {
  napi_async_runtime::try_block_on_dyn(pin!(HandoffHook::new(future)))
}
