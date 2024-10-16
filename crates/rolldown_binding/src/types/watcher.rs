use std::sync::Arc;

use napi_derive::napi;

use crate::utils::handle_result;

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
  pub async fn close(&self) -> napi::Result<()> {
    handle_result(self.inner.close().await)
  }
}
