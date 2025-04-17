use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

pub type InvalidateJsSideCacheFn =
  dyn Fn() -> Pin<Box<(dyn Future<Output = anyhow::Result<()>> + Send + 'static)>> + Send + Sync;

#[derive(Clone, Debug)]
#[debug("InvalidateJsSideCacheFn::Fn(...)")]
pub struct InvalidateJsSideCache(Arc<InvalidateJsSideCacheFn>);

impl InvalidateJsSideCache {
  pub fn new(f: Arc<InvalidateJsSideCacheFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self) -> anyhow::Result<()> {
    self.0().await
  }
}
