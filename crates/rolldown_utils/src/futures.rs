use futures::Future;

pub use crate::async_runtime::{JoinError, JoinHandle, RuntimeConfigError as SpawnError};

#[inline]
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  crate::async_runtime::spawn(future)
}

#[inline]
pub fn try_spawn<F>(future: F) -> Result<JoinHandle<F::Output>, (SpawnError, F)>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  crate::async_runtime::try_spawn(future)
}

#[inline]
pub fn spawn_detached<F>(future: F)
where
  F: Future<Output = ()> + Send + 'static,
{
  crate::async_runtime::spawn_detached(future);
}

/// Submit a fire-and-forget future without consuming it when no executor is
/// reachable; `Err` hands the future back so the caller decides its fate.
///
/// `Ok` means the future was handed to an executor, not that it will run: the
/// scheduler checks admission, so a closed scheduler returns `Err`.
#[inline]
pub fn try_spawn_detached<F>(future: F) -> Result<(), F>
where
  F: Future<Output = ()> + Send + 'static,
{
  crate::async_runtime::try_spawn_detached(future)
}

#[inline]
pub fn spawn_blocking<F, Out>(function: F) -> JoinHandle<Out>
where
  F: FnOnce() -> Out + Send + 'static,
  Out: Send + 'static,
{
  crate::async_runtime::spawn_blocking(function)
}

/// `async` here is only used to satisfy the wasm shim version of `block_on_spawn_all`.
/// This function polls non-static futures concurrently in the caller's task and waits for all of them to finish.
pub async fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
where
  Iter: Iterator,
  Out: Send + 'static,
  Iter::Item: Future<Output = Out> + Send,
{
  use futures::future::join_all;
  join_all(iter).await
}

#[expect(clippy::collection_is_never_read)]
async fn _test_block_on_spawn_all_non_static_future() {
  let mut words = String::new();
  let non_static_future = async {
    words.push_str("hello");
  };
  let _ = block_on_spawn_all(std::iter::once(non_static_future)).await;
}

pub fn block_on<F: Future>(f: F) -> F::Output {
  crate::async_runtime::block_on(f)
}

/// Whether this target may create OS threads with `std::thread::spawn`.
///
/// `std::thread::spawn` must stay off every wasm artifact: the threadless
/// `wasm32-wasip1` build has no threads at all, and the threaded build's
/// workers are owned by the napi runtime lifecycle rather than by us. Native
/// targets spawn freely, on every scheduler flavor -- this is a compile-time
/// property of the target, never a function of the runtime flavor the process
/// happens to have selected.
#[inline]
pub const fn can_spawn_os_threads() -> bool {
  cfg!(not(target_family = "wasm"))
}
