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

#[inline]
pub fn spawn_blocking<F, Out>(function: F) -> JoinHandle<Out>
where
  F: FnOnce() -> Out + Send + 'static,
  Out: Send + 'static,
{
  crate::async_runtime::spawn_blocking(function)
}

/// This function allows you to spawn non-static futures in parallel and wait
/// for all of them to finish.
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
