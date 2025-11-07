use super::Bundler;
use crate::{Build, types::bundle_output::BundleOutput};
use arcstr::ArcStr;
use rolldown_common::ScanMode;
use rolldown_error::BuildResult;
use std::mem;

impl Bundler {
  pub(crate) async fn with_incremental_build<T>(
    &mut self,
    with_fn: impl AsyncFnOnce(&mut Build) -> BuildResult<T>,
  ) -> BuildResult<T> {
    let cache = mem::take(&mut self.cache);
    let mut build = self.build_factory.create_incremental_build(cache);
    let ret = with_fn(&mut build).await?;
    self.cache = build.cache;
    Ok(ret)
  }

  pub(crate) async fn incremental_write(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<BundleOutput> {
    self.incremental_build(true, scan_mode).await
  }

  pub(crate) async fn incremental_generate(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<BundleOutput> {
    self.incremental_build(false, scan_mode).await
  }

  async fn incremental_build(
    &mut self,
    is_write: bool,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<BundleOutput> {
    self
      .with_incremental_build(async |build| {
        let middle_output = build.scan_modules(scan_mode).await?;
        if is_write {
          build.bundle_write(middle_output).await
        } else {
          build.bundle_generate(middle_output).await
        }
      })
      .await
  }
}
