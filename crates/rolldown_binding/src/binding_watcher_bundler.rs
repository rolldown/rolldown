use napi::{Env, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use rolldown::BundleHandle;

use crate::utils::spawn_boxed_future;

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
    spawn_boxed_future(env, async move {
      inner.close().await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
      Ok(())
    })
  }
}

impl BindingWatcherBundler {
  pub fn new(inner: BundleHandle) -> Self {
    Self { inner }
  }
}
