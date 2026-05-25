#[cfg(feature = "experimental")]
use super::Bundler;
#[cfg(feature = "experimental")]
use crate::{Bundle, types::bundle_output::BundleOutput};
#[cfg(feature = "experimental")]
use arcstr::ArcStr;
#[cfg(feature = "experimental")]
use rolldown_common::{BundleMode, ScanMode};
#[cfg(feature = "experimental")]
use rolldown_error::BuildResult;
#[cfg(feature = "experimental")]
use std::mem;

#[cfg(feature = "experimental")]
impl Bundler {
  pub(crate) async fn with_cached_bundle<T>(
    &mut self,
    bundle_mode: BundleMode,
    with_fn: impl AsyncFnOnce(&mut Bundle) -> BuildResult<T>,
  ) -> BuildResult<T> {
    let cache = mem::take(&mut self.cache);
    let mut bundle = self.bundle_factory.create_bundle(bundle_mode, Some(cache))?;
    // Do NOT `?` the build result here. The cache must be moved back into the
    // `Bundler` on every outcome; bailing on `Err` would drop `bundle` and leave
    // `Bundler::cache` at the `default()` (snapshot = None) that `mem::take`
    // installed above, making the next HMR cycle panic in `get_snapshot()`.
    // See meta/design/bundler-data-lifecycle.md ("Cache integrity on a failed build").
    let ret = with_fn(&mut bundle).await;
    self.cache = bundle.cache;
    ret
  }

  pub async fn with_cached_bundle_experimental<T>(
    &mut self,
    bundle_mode: BundleMode,
    with_fn: impl AsyncFnOnce(&mut Bundle) -> BuildResult<T>,
  ) -> BuildResult<T> {
    self.with_cached_bundle(bundle_mode, with_fn).await
  }

  pub async fn incremental_write(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<BundleOutput> {
    self.incremental_bundle(true, scan_mode).await
  }

  pub async fn incremental_generate(
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
    let bundle_mode = match scan_mode {
      ScanMode::Full => BundleMode::IncrementalFullBuild,
      ScanMode::Partial(_) => BundleMode::IncrementalBuild,
    };
    self
      .with_cached_bundle(bundle_mode, async |bundle| {
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
