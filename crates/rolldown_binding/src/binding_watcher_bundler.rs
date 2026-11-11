use napi::{Env, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use rolldown::BundleHandle;

use crate::types::js_callback_release::SharedJsCallbackRelease;
use crate::utils::spawn_boxed_future;

/// Minimal wrapper around a `BundleHandle` for watcher events.
/// This is returned from watcher event data to allow calling `result.close()`.
#[napi]
pub struct BindingWatcherBundler {
  inner: BundleHandle,
  /// The JS functions of the watcher options. This object never releases them.
  option_callbacks: SharedJsCallbackRelease,
}

#[napi]
impl BindingWatcherBundler {
  #[napi(ts_return_type = "Promise<void>")]
  pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = self.inner.clone();
    // `watcher.close()` does not wait for this close, which can run `closeBundle`. Take the hold
    // now, before the spawn, so that the release of the watcher waits until this close ends.
    // See internal-docs/bundler-data-lifecycle/implementation.md ("JS callback lifetime").
    let hold = self.option_callbacks.hold();
    spawn_boxed_future(env, async move {
      let _hold = hold;
      inner.close().await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
      Ok(())
    })
  }
}

impl BindingWatcherBundler {
  pub fn new(inner: BundleHandle, option_callbacks: SharedJsCallbackRelease) -> Self {
    Self { inner, option_callbacks }
  }
}
