use std::sync::Arc;

use napi::tokio::sync::Mutex;
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;

/// Minimal wrapper around the core `Bundler` for watcher events.
/// This is returned from watcher event data to allow access to the bundler instance.
#[napi]
pub struct BindingWatcherBundler {
  inner: Arc<Mutex<NativeBundler>>,
}

#[napi]
impl BindingWatcherBundler {
  /// Fully close the bundler and clean up all resources including the cache.
  /// This should be called when a build fails or when the watcher is being shut down.
  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    let mut bundler = self.inner.lock().await;
    bundler.close().await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(())
  }

  /// Close the bundle and call the `closeBundle` hook, but preserve the cache for incremental builds.
  /// This should be used in watch mode after each successful build completes.
  #[napi]
  pub async fn close_bundle(&self) -> napi::Result<()> {
    let mut bundler = self.inner.lock().await;
    bundler.close_bundle().await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(())
  }
}

impl BindingWatcherBundler {
  pub fn new(inner: Arc<Mutex<NativeBundler>>) -> Self {
    Self { inner }
  }
}
