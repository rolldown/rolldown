use futures::Future;

#[inline]
pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  tokio::spawn(future)
}

pub fn block_on<F: Future>(f: F) -> F::Output {
  #[cfg(target_family = "wasm")]
  {
    futures::executor::block_on(f)
  }
  #[cfg(not(target_family = "wasm"))]
  {
    tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(f))
  }
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
