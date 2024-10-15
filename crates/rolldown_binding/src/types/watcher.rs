use std::sync::Arc;

use napi_derive::napi;

#[napi]
pub struct BindingWatcher {
  inner: Arc<rolldown::Watcher>,
}

#[napi]
impl BindingWatcher {
  pub fn new(inner: Arc<rolldown::Watcher>) -> Self {
    Self { inner }
  }

  #[napi]
  pub async fn close(&self) {
    self.inner.close().await;
  }
}
