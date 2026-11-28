#[cfg(feature = "experimental")]
use super::Bundler;
#[cfg(feature = "experimental")]
use crate::hmr::hmr_stage::{HmrStage, HmrStageInput};
#[cfg(feature = "experimental")]
use rolldown_common::{
  ClientHave, HmrPatch, HmrPullResponse, HmrSession, HmrUpdate, SharedHmrSession, WatcherChangeKind,
};
#[cfg(feature = "experimental")]
use rolldown_error::BuildResult;
#[cfg(feature = "experimental")]
use rolldown_utils::indexmap::FxIndexMap;
#[cfg(feature = "experimental")]
use std::sync::Arc;

#[cfg(feature = "experimental")]
impl Bundler {
  #[tracing::instrument(level = "debug", skip_all)]
  /// `last_build_errored` disables the unchanged-output suppression: a recovery
  /// that rebuilds byte-identical output must still reach clients stuck on the
  /// error — see `HmrStage::compute_hmr_update_for_file_changes`.
  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &FxIndexMap<String, WatcherChangeKind>,
    last_build_errored: bool,
    hot_update_hook_enabled: bool,
  ) -> BuildResult<HmrUpdate> {
    let _activity = crate::utils::build_activity::BuildActivity::enter();
    let Some(mut hmr_stage) = self.hmr_stage() else {
      return Err(anyhow::format_err!(
        "HMR requires to run at least one bundle before invalidation"
      ))?;
    };
    hmr_stage
      .compute_hmr_update_for_file_changes(
        changed_file_paths,
        last_build_errored,
        hot_update_hook_enabled,
      )
      .await
  }

  /// Answers a client's pull, and compiles the lazy routes it asks for that are not in the graph
  /// yet. `merged` becomes `true` once such a compile merges into the graph, also when a later
  /// step fails. See `HmrStage::render_patch`.
  pub async fn render_patch(
    &mut self,
    have: &ClientHave,
    merged: &mut bool,
  ) -> BuildResult<HmrPullResponse<HmrPatch>> {
    let _activity = crate::utils::build_activity::BuildActivity::enter();
    let lazy_ctx = self.lazy_compilation_context.clone();
    let Some(mut hmr_stage) = self.hmr_stage() else {
      return Ok(HmrPullResponse::Reload { reason: "no build has run yet".to_owned() });
    };
    hmr_stage.render_patch(have, lazy_ctx.as_ref(), merged).await
  }

  /// Starts a new session with `epoch`. Call it before the first build.
  pub fn set_hmr_epoch(&mut self, epoch: u32) {
    *self.bundle_factory.hmr_session.lock() = HmrSession::new(epoch);
    // Stamps start again at 0, so a cached factory could match a module that changed since.
    self.hmr_render_cache.clear();
  }

  pub fn hmr_session(&self) -> &SharedHmrSession {
    &self.bundle_factory.hmr_session
  }

  /// See `HmrSession::begin_full_reload`.
  pub fn begin_hmr_full_reload(&mut self) {
    self.bundle_factory.hmr_session.lock().begin_full_reload();
    // A full reload follows a graph change that no stamp records, such as a lost update.
    self.hmr_render_cache.clear();
  }

  #[cfg(feature = "testing")]
  pub fn hmr_render_cache_hits_for_testing(&self) -> crate::RenderCacheHits {
    self.hmr_render_cache.hits()
  }

  /// Leaves the graph as a failure after the merge of an HMR update does: with an edit that no
  /// client received.
  #[cfg(feature = "testing")]
  pub fn lose_hmr_update_for_testing(&mut self) {
    self.lost_hmr_update = true;
  }

  fn hmr_stage(&mut self) -> Option<HmrStage<'_, rolldown_fs::OsFileSystem>> {
    // HMR partial scans use the shared rayon pool without passing through
    // `BundleFactory::build_bundle`; wait for any deferred drops here too.
    crate::utils::defer_drop::drain();

    let plugin_driver = self.last_bundle_handle.as_ref().map(|ctx| &ctx.plugin_driver)?;
    Some(HmrStage::new(HmrStageInput {
      fs: self.bundle_factory.fs.clone(),
      options: Arc::clone(&self.bundle_factory.options),
      resolver: Arc::clone(&self.bundle_factory.resolver),
      plugin_driver: Arc::clone(plugin_driver),
      cache: &mut self.cache,
      lost_hmr_update: &mut self.lost_hmr_update,
      session: self.bundle_factory.hmr_session.clone(),
      render_cache: &mut self.hmr_render_cache,
    }))
  }
}

#[cfg(all(test, feature = "experimental"))]
mod tests {
  use std::sync::Arc;

  use crate::{Bundler, BundlerOptions, hmr::render_cache::RenderedFactory};

  #[test]
  fn a_new_epoch_clears_the_render_cache() {
    let mut bundler = Bundler::new(BundlerOptions::default()).unwrap();
    let factory = Arc::new(RenderedFactory { code: "x".to_string(), map: None });
    bundler.hmr_render_cache.insert("a.js".into(), 1, 0, factory);
    bundler.set_hmr_epoch(2);
    assert!(bundler.hmr_render_cache.get("a.js", 1, 0).is_none());
  }

  #[test]
  fn a_full_reload_clears_the_render_cache() {
    let mut bundler = Bundler::new(BundlerOptions::default()).unwrap();
    let factory = Arc::new(RenderedFactory { code: "x".to_string(), map: None });
    bundler.hmr_render_cache.insert("a.js".into(), 1, 0, factory);
    bundler.begin_hmr_full_reload();
    assert!(bundler.hmr_render_cache.get("a.js", 1, 0).is_none());
  }
}
