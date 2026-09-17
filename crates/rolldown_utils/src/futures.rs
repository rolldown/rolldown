use futures::Future;

#[inline]
pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  tokio::spawn(future)
}

/// Blocks the current thread until `f` completes.
///
/// Only call this function when `f` can make progress on this thread alone. Two
/// kinds of work need another thread. A spawned task needs a worker thread to
/// poll it. A JS callback that re-enters rolldown needs the runtime too. `f`
/// then never completes, and the build deadlocks (#10664).
///
/// On targets that are not wasm, this function calls `block_in_place`. That
/// function moves this worker thread's scheduler work to the blocking pool.
/// `ROLLDOWN_MAX_BLOCKING_THREADS` limits that pool to 4 threads by default.
/// When the pool is full, no thread remains to run the scheduler work.
///
/// On wasm, this function calls `futures::executor::block_on`. That function
/// polls `f` on this thread and holds the thread. The same rule applies.
///
/// Use `.await` in place of this function whenever you can.
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
