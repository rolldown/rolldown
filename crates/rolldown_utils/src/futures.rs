use futures::Future;

/// `async` here is only used to satisfy the wasm shim version of `block_on_spawn_all`.
/// This function allow you to spawn non-static futures in parallel and wait for all of them to finish.
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::unused_async)]
pub async fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
where
  Iter: Iterator,
  Out: Send + 'static,
  Iter::Item: Future<Output = Out> + Send,
{
  use async_scoped::TokioScope;
  let (_ret, collections) =
    async_scoped::Scope::scope_and_block(|scope: &mut TokioScope<'_, _>| {
      iter.into_iter().for_each(|fut| scope.spawn(fut));
    });
  collections.into_iter().map(Result::unwrap).collect()
}

async fn _test_block_on_spawn_all_non_static_future() {
  let mut words = String::new();
  let non_static_future = async {
    words.push_str("hello");
  };
  let _ = block_on_spawn_all(std::iter::once(non_static_future)).await;
}

#[cfg(target_arch = "wasm32")]
pub async fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
where
  Iter: Iterator,
  Out: Send + 'static,
  Iter::Item: Future<Output = Out> + Send,
{
  use futures::future::join_all;
  let outs = join_all(iter).await;
  outs
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
