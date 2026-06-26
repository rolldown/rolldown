use futures::Future;

#[cfg(feature = "async-runtime")]
pub use crate::async_runtime::{JoinError, JoinHandle};

#[cfg(not(feature = "async-runtime"))]
pub type JoinHandle<T> = tokio::task::JoinHandle<T>;

#[inline]
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  #[cfg(feature = "async-runtime")]
  {
    crate::async_runtime::spawn(future)
  }
  #[cfg(not(feature = "async-runtime"))]
  {
    tokio::spawn(future)
  }
}

#[inline]
pub fn spawn_detached<F>(future: F)
where
  F: Future<Output = ()> + Send + 'static,
{
  #[cfg(feature = "async-runtime")]
  {
    crate::async_runtime::spawn_detached(future);
  }
  #[cfg(not(feature = "async-runtime"))]
  {
    drop(tokio::spawn(future));
  }
}

#[inline]
pub fn spawn_blocking<F, Out>(function: F) -> JoinHandle<Out>
where
  F: FnOnce() -> Out + Send + 'static,
  Out: Send + 'static,
{
  #[cfg(feature = "async-runtime")]
  {
    crate::async_runtime::spawn_blocking(function)
  }
  #[cfg(not(feature = "async-runtime"))]
  {
    tokio::task::spawn_blocking(function)
  }
}

/// `async` here is only used to satisfy the wasm shim version of `block_on_spawn_all`.
/// This function allow you to spawn non-static futures in parallel and wait for all of them to finish.
#[cfg_attr(
  all(not(feature = "async-runtime"), not(target_arch = "wasm32")),
  expect(clippy::unused_async)
)]
pub async fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
where
  Iter: Iterator,
  Out: Send + 'static,
  Iter::Item: Future<Output = Out> + Send,
{
  #[cfg(any(feature = "async-runtime", target_arch = "wasm32"))]
  {
    use futures::future::join_all;
    join_all(iter).await
  }
  #[cfg(all(not(feature = "async-runtime"), not(target_arch = "wasm32")))]
  {
    use async_scoped::TokioScope;
    let (_ret, collections) =
      async_scoped::Scope::scope_and_block(|scope: &mut TokioScope<'_, _>| {
        iter.into_iter().for_each(|fut| scope.spawn(fut));
      });
    collections.into_iter().map(Result::unwrap).collect()
  }
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
  #[cfg(feature = "async-runtime")]
  {
    crate::async_runtime::block_on(f)
  }
  #[cfg(all(not(feature = "async-runtime"), target_family = "wasm"))]
  {
    futures::executor::block_on(f)
  }
  #[cfg(all(not(feature = "async-runtime"), not(target_family = "wasm")))]
  {
    tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(f))
  }
}
