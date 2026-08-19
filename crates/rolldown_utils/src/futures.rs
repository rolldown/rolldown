use futures::Future;

#[cfg(not(feature = "tokio-runtime"))]
pub use crate::async_runtime::{JoinError, JoinHandle, RuntimeConfigError as SpawnError};

#[cfg(feature = "tokio-runtime")]
pub type JoinHandle<T> = tokio::task::JoinHandle<T>;
#[cfg(feature = "tokio-runtime")]
pub type SpawnError = std::convert::Infallible;

#[inline]
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  #[cfg(not(feature = "tokio-runtime"))]
  {
    crate::async_runtime::spawn(future)
  }
  #[cfg(feature = "tokio-runtime")]
  {
    tokio::spawn(future)
  }
}

#[inline]
#[cfg_attr(
  feature = "tokio-runtime",
  expect(
    clippy::unnecessary_wraps,
    reason = "the Tokio and shared-runtime spawn facades must expose one feature-stable signature"
  )
)]
pub fn try_spawn<F>(future: F) -> Result<JoinHandle<F::Output>, (SpawnError, F)>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  #[cfg(not(feature = "tokio-runtime"))]
  {
    crate::async_runtime::try_spawn(future)
  }
  #[cfg(feature = "tokio-runtime")]
  {
    Ok(tokio::spawn(future))
  }
}

#[inline]
pub fn spawn_detached<F>(future: F)
where
  F: Future<Output = ()> + Send + 'static,
{
  #[cfg(not(feature = "tokio-runtime"))]
  {
    crate::async_runtime::spawn_detached(future);
  }
  #[cfg(feature = "tokio-runtime")]
  {
    drop(tokio::spawn(future));
  }
}

/// Submit a fire-and-forget future without consuming it when no executor is
/// reachable; `Err` hands the future back so the caller decides its fate.
///
/// `Ok` means the future was handed to an executor, not that it will run.
/// The shared-runtime arm checks admission, so a closed scheduler returns
/// `Err`. The tokio arm cannot observe admission: `Handle::spawn` during
/// runtime shutdown returns a handle for a task that is cancelled before it
/// polls, so a shutdown race yields `Ok` while the future is dropped by the
/// runtime. Callers that need submit-or-get-back semantics must treat `Ok`
/// as at-most-once and put their cleanup in the future's `Drop` path — the
/// module-loader task supervisor does exactly that.
#[inline]
pub fn try_spawn_detached<F>(future: F) -> Result<(), F>
where
  F: Future<Output = ()> + Send + 'static,
{
  #[cfg(not(feature = "tokio-runtime"))]
  {
    crate::async_runtime::try_spawn_detached(future)
  }
  #[cfg(feature = "tokio-runtime")]
  {
    match tokio::runtime::Handle::try_current() {
      Ok(handle) => {
        drop(handle.spawn(future));
        Ok(())
      }
      Err(_) => Err(future),
    }
  }
}

#[inline]
pub fn spawn_blocking<F, Out>(function: F) -> JoinHandle<Out>
where
  F: FnOnce() -> Out + Send + 'static,
  Out: Send + 'static,
{
  #[cfg(not(feature = "tokio-runtime"))]
  {
    crate::async_runtime::spawn_blocking(function)
  }
  #[cfg(feature = "tokio-runtime")]
  {
    tokio::task::spawn_blocking(function)
  }
}

/// `async` here is only used to satisfy the wasm shim version of `block_on_spawn_all`.
/// This function allow you to spawn non-static futures in parallel and wait for all of them to finish.
#[cfg_attr(
  all(feature = "tokio-runtime", not(target_arch = "wasm32")),
  expect(clippy::unused_async)
)]
pub async fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
where
  Iter: Iterator,
  Out: Send + 'static,
  Iter::Item: Future<Output = Out> + Send,
{
  #[cfg(any(not(feature = "tokio-runtime"), target_arch = "wasm32"))]
  {
    use futures::future::join_all;
    join_all(iter).await
  }
  #[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
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
  #[cfg(not(feature = "tokio-runtime"))]
  {
    crate::async_runtime::block_on(f)
  }
  #[cfg(all(feature = "tokio-runtime", target_family = "wasm"))]
  {
    futures::executor::block_on(f)
  }
  #[cfg(all(feature = "tokio-runtime", not(target_family = "wasm")))]
  {
    tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(f))
  }
}

/// Whether the selected executor runs the multi-thread flavor.
#[inline]
pub fn is_multi_threaded() -> bool {
  #[cfg(not(feature = "tokio-runtime"))]
  {
    crate::async_runtime::is_multi_threaded()
  }
  #[cfg(feature = "tokio-runtime")]
  {
    match tokio::runtime::Handle::try_current() {
      Ok(handle) => handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread,
      // No entered runtime: mirror the shared scheduler, which answers from its
      // configured options — MultiThread on native, CurrentThread on wasm.
      Err(_) => !cfg!(target_family = "wasm"),
    }
  }
}
