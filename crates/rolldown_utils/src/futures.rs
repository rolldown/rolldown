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
