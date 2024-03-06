pub mod reserved_word;

use std::future::Future;

#[cfg(not(target_arch = "wasm32"))]
pub fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
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

#[cfg(target_arch = "wasm32")]
pub fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
where
  Iter: Iterator,
  Out: Send + 'static,
  Iter::Item: Future<Output = Out> + Send,
{
  use futures::{executor::block_on, future};
  block_on(future::join_all(iter))
}
