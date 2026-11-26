use std::fmt;

use futures::Future;

pub use tokio::task::{JoinError, JoinHandle};

#[derive(Debug)]
pub struct SpawnError;

impl fmt::Display for SpawnError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("no active async runtime handle")
  }
}

impl std::error::Error for SpawnError {}

#[inline]
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  tokio::spawn(future)
}

#[inline]
pub fn spawn_detached<F>(future: F)
where
  F: Future<Output = ()> + Send + 'static,
{
  drop(tokio::spawn(future));
}

#[inline]
pub fn try_spawn<F>(future: F) -> Result<JoinHandle<F::Output>, (SpawnError, F)>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  match tokio::runtime::Handle::try_current() {
    Ok(handle) => Ok(handle.spawn(future)),
    Err(_) => Err((SpawnError, future)),
  }
}

/// Submit a fire-and-forget future without consuming it when no executor is
/// reachable; `Err` hands the future back so the caller decides its fate.
#[inline]
pub fn try_spawn_detached<F>(future: F) -> Result<(), (SpawnError, F)>
where
  F: Future<Output = ()> + Send + 'static,
{
  match tokio::runtime::Handle::try_current() {
    Ok(handle) => {
      drop(handle.spawn(future));
      Ok(())
    }
    Err(_) => Err((SpawnError, future)),
  }
}

#[inline]
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
  F: FnOnce() -> R + Send + 'static,
  R: Send + 'static,
{
  tokio::task::spawn_blocking(f)
}

/// Whether Rolldown starts its own offload threads with `std::thread::spawn`,
/// such as the scan stage's sourcemap drainer.
///
/// False on every wasm artifact, where that work runs inline. Threadless
/// `wasm32-wasip1` cannot spawn a thread at all. `wasm32-wasip1-threads` can,
/// but each spawn takes a Worker from the loader's pool or starts one more,
/// and the gain of a drainer there is not measured, so that build runs inline
/// too. A property of the target, never of the selected scheduler flavor.
#[inline]
pub const fn can_spawn_os_threads() -> bool {
  cfg!(not(target_family = "wasm"))
}
