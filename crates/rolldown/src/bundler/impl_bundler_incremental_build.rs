use super::Bundler;
use crate::{Bundle, types::bundle_output::BundleOutput};
use arcstr::ArcStr;
use rolldown_common::ScanMode;
use rolldown_error::BuildResult;
use std::mem;

impl Bundler {
  pub(crate) async fn with_incremental_bundle<T>(
    &mut self,
    with_fn: impl AsyncFnOnce(&mut Bundle) -> BuildResult<T>,
  ) -> BuildResult<T> {
    let cache = mem::take(&mut self.cache);
    let mut bundle = self.bundle_factory.create_incremental_bundle(cache);
    let ret = with_fn(&mut bundle).await?;
    self.cache = bundle.cache;
    Ok(ret)
  }

  pub(crate) async fn incremental_write(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<BundleOutput> {
    self.incremental_bundle(true, scan_mode).await
  }

  pub(crate) async fn incremental_generate(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<BundleOutput> {
    self.incremental_bundle(false, scan_mode).await
  }

  async fn incremental_bundle(
    &mut self,
    is_write: bool,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<BundleOutput> {
    self
      .with_incremental_bundle(async |bundle| {
        let middle_output = bundle.scan_modules(scan_mode).await?;
        if is_write {
          bundle.bundle_write(middle_output).await
        } else {
          bundle.bundle_generate(middle_output).await
        }
      })
      .await
  }
}
