use napi::{Env, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use rolldown::BundleHandle;

use crate::utils::spawn_boxed_future;

fn handle_close_result(result: anyhow::Result<()>) -> napi::Result<()> {
  result.map_err(|error| {
    if let Some(error) = error.chain().find_map(|cause| cause.downcast_ref::<napi::Error>()) {
      return error.try_clone().unwrap_or_else(|clone_error| clone_error);
    }
    napi::Error::from_reason(error.to_string())
  })
}

/// Minimal wrapper around a `BundleHandle` for watcher events.
/// This is returned from watcher event data to allow calling `result.close()`.
#[napi]
pub struct BindingWatcherBundler {
  inner: BundleHandle,
}

#[napi]
impl BindingWatcherBundler {
  #[napi(ts_return_type = "Promise<void>")]
  pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = self.inner.clone();
    spawn_boxed_future(env, async move { handle_close_result(inner.close().await) })
  }
}

impl BindingWatcherBundler {
  pub fn new(inner: BundleHandle) -> Self {
    Self { inner }
  }
}
