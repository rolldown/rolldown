use futures::Future;

/// Fire-and-forget spawn for a `'static` future on a dedicated OS thread.
///
/// Runtime-agnostic: `std::thread::spawn` + `pollster::block_on`. Suitable
/// for low-frequency, fire-and-forget work (e.g. invoking a logging
/// callback) — do NOT use for hot-path parallelism.
#[inline]
pub fn spawn<F>(future: F) -> std::thread::JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  std::thread::spawn(move || pollster::block_on(future))
}

/// Drive many non-`'static` futures concurrently on the awaiter's task and
/// collect their outputs.
///
/// Runtime-agnostic: backed by `futures::future::join_all`, which polls every
/// future cooperatively without spawning. The previous implementation used
/// `async_scoped::TokioScope`, which spawned each future on a tokio worker
/// for cross-thread parallelism. Call sites are small per-item futures (a few
/// specifier resolves at most) and the surrounding work is already on a
/// rayon worker, so single-task concurrency is sufficient.
pub async fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
where
  Iter: Iterator,
  Out: Send + 'static,
  Iter::Item: Future<Output = Out> + Send,
{
  futures::future::join_all(iter).await
}

#[expect(clippy::collection_is_never_read)]
async fn _test_block_on_spawn_all_non_static_future() {
  let mut words = String::new();
  let non_static_future = async {
    words.push_str("hello");
  };
  let _ = block_on_spawn_all(std::iter::once(non_static_future)).await;
}

/// Synchronously drive a future to completion on the current thread.
///
/// Backed by `pollster`, a tiny single-thread parker. Runtime-agnostic — no
/// tokio runtime is required in scope — and reentrant, so call sites inside
/// sync fns can `block_on` a future even if the calling thread is already
/// inside another `block_on`.
pub fn block_on<F: Future>(f: F) -> F::Output {
  pollster::block_on(f)
}
