use derive_more::Debug;
use std::{future::Future, pin::Pin, sync::Arc};

use crate::DeferSyncScanData;

type DeferSyncScanDataInner = dyn Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<DeferSyncScanData>>> + Send + 'static>>
  + Send
  + Sync
  + 'static;

#[derive(Clone, Debug)]
#[debug("DeferSyncScanDataOption::Fn(...)")]
pub struct DeferSyncScanDataOption(Arc<DeferSyncScanDataInner>);

impl DeferSyncScanDataOption {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<DeferSyncScanData>>> + Send + 'static>>
      + Send
      + Sync
      + 'static,
  {
    Self(Arc::new(f))
  }

  pub async fn exec(&self) -> anyhow::Result<Vec<DeferSyncScanData>> {
    let t = self.0();
    t.await
  }
}
