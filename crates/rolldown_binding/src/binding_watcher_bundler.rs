use napi_derive::napi;
use rolldown::BundleHandle;

/// Minimal wrapper around a `BundleHandle` for watcher events.
/// This is returned from watcher event data to allow calling `result.close()`.
#[napi]
pub struct BindingWatcherBundler {
  inner: BundleHandle,
}

#[napi]
impl BindingWatcherBundler {
  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    self.inner.close().await.map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(())
  }
}

impl BindingWatcherBundler {
  pub fn new(inner: BundleHandle) -> Self {
    Self { inner }
  }
}
