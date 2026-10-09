use futures::Future;

// These forward to the runtime unchanged. `spawn` and `try_spawn` below stay
// wrappers only because they take one generic parameter (`<F>`) where the
// runtime's take two (`<F, T>`).
// See internal-docs/async-runtime/implementation.md, "Rust core: facades, module loader, Rayon".
pub use crate::async_runtime::{
  JoinError, JoinHandle, RuntimeConfigError as SpawnError, block_on, spawn_blocking, spawn_detached,
};

/// Submit a fire-and-forget future without consuming it when no executor is
/// reachable; `Err` hands the future back so the caller decides its fate.
///
/// `Ok` means the future was handed to an executor, not that it will run: the
/// scheduler checks admission, so a closed scheduler returns `Err`.
pub use crate::async_runtime::try_spawn_detached;

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

/// Polls non-static futures concurrently in the caller's task and waits for all of them to finish.
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

/// Whether Rolldown starts its own offload threads with `std::thread::spawn`,
/// such as the scan stage's sourcemap drainer.
///
/// False on every wasm artifact, where that work runs inline; threadless
/// `wasm32-wasip1` cannot spawn a thread at all. A property of the target,
/// never of the selected scheduler flavor.
#[inline]
pub const fn can_spawn_os_threads() -> bool {
  cfg!(not(target_family = "wasm"))
}
