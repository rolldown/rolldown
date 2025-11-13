use super::Bundler;
use crate::hmr::hmr_stage::{HmrStage, HmrStageInput};
use rolldown_common::{ClientHmrInput, ClientHmrUpdate, HmrUpdate};
use rolldown_error::BuildResult;
use rustc_hash::FxHashSet;
use std::sync::{Arc, atomic::AtomicU32};

impl Bundler {
  #[cfg(feature = "experimental")]
  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &[String],
    clients: &[ClientHmrInput<'_>],
    next_hmr_patch_id: Arc<AtomicU32>,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    let Some(plugin_driver) = self.last_bundle_handle.as_ref().map(|ctx| &ctx.plugin_driver) else {
      return Err(anyhow::format_err!(
        "HMR requires to run at least one bundle before invalidation"
      ))?;
    };
    let mut hmr_stage = HmrStage::new(HmrStageInput {
      fs: self.bundle_factory.fs.clone(),
      options: Arc::clone(&self.bundle_factory.options),
      resolver: Arc::clone(&self.bundle_factory.resolver),
      plugin_driver: Arc::clone(plugin_driver),
      cache: &mut self.cache,
      next_hmr_patch_id,
    });
    hmr_stage.compute_hmr_update_for_file_changes(changed_file_paths, clients).await
  }

  #[cfg(feature = "experimental")]
  pub async fn compute_update_for_calling_invalidate(
    &mut self,
    invalidate_caller: String,
    first_invalidated_by: Option<String>,
    client_id: &str,
    executed_modules: &FxHashSet<String>,
    next_hmr_patch_id: Arc<AtomicU32>,
  ) -> BuildResult<HmrUpdate> {
    let Some(plugin_driver) = self.last_bundle_handle.as_ref().map(|ctx| &ctx.plugin_driver) else {
      return Err(anyhow::format_err!(
        "HMR requires to run at least one bundle before invalidation"
      ))?;
    };
    let mut hmr_stage = HmrStage::new(HmrStageInput {
      fs: self.bundle_factory.fs.clone(),
      options: Arc::clone(&self.bundle_factory.options),
      resolver: Arc::clone(&self.bundle_factory.resolver),
      plugin_driver: Arc::clone(plugin_driver),
      cache: &mut self.cache,
      next_hmr_patch_id,
    });
    hmr_stage
      .compute_update_for_calling_invalidate(
        invalidate_caller,
        first_invalidated_by,
        client_id,
        executed_modules,
      )
      .await
  }
}
