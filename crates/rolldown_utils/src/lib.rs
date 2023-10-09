use std::future::Future;

use async_scoped::TokioScope;
pub mod reserved_word;
mod untrust;

pub fn block_on_spawn_all<Iter, Out>(iter: Iter) -> Vec<Out>
where
  Iter: Iterator,
  Out: Send + 'static,
  Iter::Item: Future<Output = Out> + Send,
{
  let (_ret, collections) =
    async_scoped::Scope::scope_and_block(|scope: &mut TokioScope<'_, _>| {
      iter.into_iter().for_each(|fut| scope.spawn(fut));
    });
  collections.into_iter().map(|item| item.unwrap()).collect()
}
