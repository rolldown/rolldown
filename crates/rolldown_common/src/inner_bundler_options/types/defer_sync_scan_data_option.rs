use derive_more::Debug;
use rolldown_error::SingleBuildResult;
use std::{future::Future, pin::Pin, sync::Arc};

use crate::DeferSyncScanData;

type DeferSyncScanDataInner = dyn Fn() -> Pin<Box<dyn Future<Output = SingleBuildResult<Vec<DeferSyncScanData>>> + Send + 'static>>
  + Send
  + Sync
  + 'static;

#[derive(Clone, Debug)]
#[debug("DeferSyncScanDataOption::Fn(...)")]
pub struct DeferSyncScanDataOption(Arc<DeferSyncScanDataInner>);

impl DeferSyncScanDataOption {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn() -> Pin<
        Box<dyn Future<Output = SingleBuildResult<Vec<DeferSyncScanData>>> + Send + 'static>,
      > + Send
      + Sync
      + 'static,
  {
    Self(Arc::new(f))
  }

  pub async fn exec(&self) -> SingleBuildResult<Vec<DeferSyncScanData>> {
    self.0().await
  }
}
