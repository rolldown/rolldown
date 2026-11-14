use derive_more::Debug;
use std::{future::Future, pin::Pin, sync::Arc};

use crate::{DeferSyncScanData, SavedSideEffects};

type DeferSyncScanDataFuture =
  Pin<Box<dyn Future<Output = anyhow::Result<Vec<DeferSyncScanData>>> + Send + 'static>>;

type DeferSyncScanDataInner =
  dyn Fn(Vec<SavedSideEffects>) -> DeferSyncScanDataFuture + Send + Sync + 'static;

#[derive(Clone, Debug)]
#[debug("DeferSyncScanDataOption::Fn(...)")]
pub struct DeferSyncScanDataOption(Arc<DeferSyncScanDataInner>);

impl DeferSyncScanDataOption {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(Vec<SavedSideEffects>) -> DeferSyncScanDataFuture + Send + Sync + 'static,
  {
    Self(Arc::new(f))
  }

  /// `saved_side_effects` are the `moduleSideEffects` values from saved results that the
  /// persistent cache reused. The JavaScript side may not have them yet, and it decides which
  /// of them are newer than its own values. Empty without the persistent cache.
  pub async fn exec(
    &self,
    saved_side_effects: Vec<SavedSideEffects>,
  ) -> anyhow::Result<Vec<DeferSyncScanData>> {
    let t = self.0(saved_side_effects);
    t.await
  }
}
