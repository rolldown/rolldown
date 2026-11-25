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
use std::{mem, sync::atomic::Ordering};
#[cfg(feature = "experimental")]
use sugar_path::SugarPath;

#[cfg(feature = "experimental")]
impl Bundler {
  pub(crate) async fn with_cached_bundle<T>(
    &mut self,
    bundle_mode: BundleMode,
    with_fn: impl AsyncFnOnce(&mut Bundle) -> BuildResult<T>,
  ) -> BuildResult<T> {
    // A failed scan never reaches `bundle_write`/`bundle_generate`, which otherwise clear this
    // cache. Load promises must not outlive the build whose native module infos they refer to.
    if let Some(invalidate_js_side_cache) = &self.options().invalidate_js_side_cache {
      invalidate_js_side_cache.call().await?;
    }
    let cache = mem::take(&mut self.cache);
    let mut bundle = self.bundle_factory.create_bundle(bundle_mode, Some(cache))?;
    // Do NOT `?` the build result here. The cache must be moved back into the
    // `Bundler` on every outcome; bailing on `Err` would drop `bundle` and leave
    // `Bundler::cache` at the `default()` (snapshot = None) that `mem::take`
    // installed above, making the next HMR cycle panic in `get_snapshot()`.
    // See internal-docs/bundler-data-lifecycle/implementation.md ("Cache integrity on a failed build").
    let ret = with_fn(&mut bundle).await;
    self.cache = bundle.cache;
    ret
  }

  pub async fn with_cached_bundle_experimental<T>(
    &mut self,
    bundle_mode: BundleMode,
    with_fn: impl AsyncFnOnce(&mut Bundle) -> BuildResult<T>,
  ) -> BuildResult<T> {
    let (result, close_hooks_completed) = self
      .with_cached_bundle(bundle_mode, async |bundle| {
        Ok((with_fn(bundle).await, bundle.close_hooks_completed))
      })
      .await?;
    let handle = self.last_bundle_handle.as_ref().expect("bundle was created");
    handle.closed.fetch_or(close_hooks_completed, Ordering::SeqCst);
    result
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
    // No snapshot, no graph to build on: scan fully. Deciding before the
    // `BundleMode` choice applies the `IncrementalFullBuild` reset rules to
    // the plugin driver state.
    let scan_mode = if self.cache.has_snapshot() { scan_mode } else { ScanMode::Full };
    let is_full_scan = scan_mode.is_full();
    self.forget_rendered_factories(&scan_mode);
    let bundle_mode = match scan_mode {
      ScanMode::Full => BundleMode::IncrementalFullBuild,
      ScanMode::Partial(_) => BundleMode::IncrementalBuild,
    };
    let output = self
      .with_cached_bundle(bundle_mode, async |bundle| {
        let middle_output = bundle.scan_modules(scan_mode).await?;
        if is_write {
          bundle.bundle_write(middle_output).await
        } else {
          bundle.bundle_generate(middle_output).await
        }
      })
      .await;
    if is_full_scan && output.is_ok() {
      // The new output has the lost edit, and the task that started this build told every
      // client to reload; see internal-docs/bundler-data-lifecycle/implementation.md.
      self.lost_hmr_update = false;
    }
    output
  }

  /// A full scan can change any module without a stamp, so every rendered HMR factory goes. A
  /// partial scan reads again its paths and both queues of earlier failures, so only their
  /// factories go.
  fn forget_rendered_factories(&mut self, scan_mode: &ScanMode<ArcStr>) {
    let ScanMode::Partial(paths) = scan_mode else {
      self.hmr_render_cache.clear();
      return;
    };
    let cache = &self.cache;
    let changed = paths.iter().filter_map(|path| {
      cache.module_idx_by_abs_path.get(&ArcStr::from(path.as_str().to_slash())).copied()
    });
    let failed_builds = cache.pending_build_rescans.iter().filter_map(|resolved_id| {
      cache.module_id_to_idx.get(&resolved_id.id).map(|state| state.idx())
    });
    self.hmr_render_cache.forget_rescanned(cache, changed.chain(failed_builds));
  }
}
