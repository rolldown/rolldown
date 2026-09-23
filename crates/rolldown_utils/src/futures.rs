use futures::Future;

// These forward to the runtime unchanged. `spawn` and `try_spawn` below stay
// wrappers only because they take one generic parameter (`<F>`) where the
// runtime's take two (`<F, T>`).
// See internal-docs/async-runtime/implementation.md, section 2.
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

/// A task future kept until it is submitted, then the handle its submission
/// produced.
///
/// [`RetainedStart::try_start`] submits the kept future at most once. An
/// accepted submission stores the handle. A rejected one (e.g. the async
/// runtime refused the spawn) puts the future back, so a later call can retry
/// after a runtime restart.
/// See internal-docs/watch-mode/implementation.md for retry ownership.
pub struct RetainedStart<P, H> {
  /// The future, before an accepted submission.
  pub pending: Option<P>,
  /// The handle, after an accepted submission.
  pub handle: Option<H>,
}

impl<P, H> RetainedStart<P, H> {
  pub fn new(pending: P) -> Self {
    Self { pending: Some(pending), handle: None }
  }

  /// Submit the kept future through `start`. Does nothing when no future is
  /// kept (already started, or taken by the owner).
  pub fn try_start<E>(&mut self, start: impl FnOnce(P) -> Result<H, (E, P)>) -> Result<(), E> {
    let Some(pending) = self.pending.take() else {
      return Ok(());
    };

    match start(pending) {
      Ok(handle) => {
        self.handle = Some(handle);
        Ok(())
      }
      Err((error, pending)) => {
        self.pending = Some(pending);
        Err(error)
      }
    }
  }
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
