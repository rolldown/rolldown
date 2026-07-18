#[cfg(feature = "experimental")]
use super::Bundler;
#[cfg(feature = "experimental")]
use crate::hmr::hmr_stage::{HmrStage, HmrStageInput};
#[cfg(feature = "experimental")]
use arcstr::ArcStr;
#[cfg(feature = "experimental")]
use rolldown_common::WatcherChangeKind;
#[cfg(feature = "experimental")]
use rolldown_common::{
  ClientHmrInput, ClientHmrUpdate, HmrLazyChunkOutput, HmrStampTable, ImportKind, Module,
};
#[cfg(feature = "experimental")]
use rolldown_error::BuildResult;
#[cfg(feature = "experimental")]
use rolldown_utils::indexmap::FxIndexMap;
#[cfg(feature = "experimental")]
use rustc_hash::FxHashMap;
#[cfg(feature = "experimental")]
use std::sync::{Arc, atomic::AtomicU32};

#[cfg(feature = "experimental")]
impl Bundler {
  #[tracing::instrument(level = "debug", skip_all)]
  /// `last_build_errored` disables the unchanged-output suppression: a recovery
  /// that rebuilds byte-identical output must still reach clients stuck on the
  /// error — see `HmrStage::compute_hmr_update_for_file_changes`.
  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &FxIndexMap<String, WatcherChangeKind>,
    clients: &[ClientHmrInput<'_>],
    stamp_table: &mut HmrStampTable,
    next_hmr_patch_id: Arc<AtomicU32>,
    last_build_errored: bool,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    // HMR partial scans use the shared rayon pool without passing through
    // `BundleFactory::build_bundle`; wait for any deferred drops here too.
    crate::utils::defer_drop::drain();

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
      .compute_hmr_update_for_file_changes(
        changed_file_paths,
        clients,
        stamp_table,
        last_build_errored,
      )
      .await
  }

  /// Compute the top-level-evaluated set of the current snapshot: the modules whose
  /// evaluation is unconditionally triggered by entry-chunk top-level execution.
  /// These are the modules reachable from the user-defined entry through
  /// `ImportKind::Import` edges only — static `import` / `export … from` statements,
  /// whose init calls the renderer hoists unconditionally into the importer.
  /// `Require` and dynamic edges are excluded: a `require()` site may be
  /// conditional, so counting it could mark a module "evaluated" that never ran —
  /// the one unsafe direction. Missing a module here only costs re-shipped bytes.
  ///
  /// Values are the modules' render-time stamps at computation, so consumers
  /// compare currency with `HmrStampTable::is_stale` exactly like the ship map.
  ///
  /// Returns an empty (inert) map when there is no snapshot or when the build has
  /// several user-defined entries: the server cannot yet tell which entry a given
  /// client loaded, and a union would mark modules evaluated for clients that
  /// never loaded them.
  pub fn compute_top_level_evaluated_modules(
    &self,
    stamp_table: &HmrStampTable,
  ) -> FxHashMap<ArcStr, u32> {
    let Some(snapshot) = self.cache.snapshot() else {
      return FxHashMap::default();
    };
    if snapshot.user_defined_entry_modules.len() != 1 {
      return FxHashMap::default();
    }

    let modules = &snapshot.module_table.modules;
    let mut evaluated = FxHashMap::default();
    let mut stack: Vec<_> = snapshot.user_defined_entry_modules.iter().copied().collect();
    while let Some(module_idx) = stack.pop() {
      let Module::Normal(module) = &modules[module_idx] else {
        continue;
      };
      let stable_id = module.stable_id.as_arc_str();
      if evaluated.contains_key(stable_id) {
        continue;
      }
      evaluated.insert(stable_id.clone(), stamp_table.render_time_stamp(stable_id.as_str()));
      for rec in &module.import_records {
        if rec.kind == ImportKind::Import
          && let Some(dep_idx) = rec.resolved_module
        {
          stack.push(dep_idx);
        }
      }
    }
    evaluated
  }

  /// Compile a lazy entry module and return compiled code plus the pending-payload
  /// entry the delivery-time ship-map write consumes.
  ///
  /// This is called when a dynamically imported module is first requested at runtime.
  /// The module was previously stubbed with a proxy, and now we need to compile the
  /// actual module and its dependencies.
  pub async fn compile_lazy_entry(
    &mut self,
    module_id: String,
    client_id: &str,
    shipped: &FxHashMap<ArcStr, u32>,
    evaluated: &FxHashMap<ArcStr, u32>,
    stamp_table: &HmrStampTable,
    next_hmr_patch_id: Arc<AtomicU32>,
  ) -> BuildResult<HmrLazyChunkOutput> {
    // HMR partial scans use the shared rayon pool without passing through
    // `BundleFactory::build_bundle`; wait for any deferred drops here too.
    crate::utils::defer_drop::drain();

    let Some(plugin_driver) = self.last_bundle_handle.as_ref().map(|ctx| &ctx.plugin_driver) else {
      panic!("Lazy compilation requires at least one bundle to be built first");
    };
    let mut hmr_stage = HmrStage::new(HmrStageInput {
      fs: self.bundle_factory.fs.clone(),
      options: Arc::clone(&self.bundle_factory.options),
      resolver: Arc::clone(&self.bundle_factory.resolver),
      plugin_driver: Arc::clone(plugin_driver),
      cache: &mut self.cache,
      next_hmr_patch_id,
    });
    hmr_stage.compile_lazy_entry(&module_id, client_id, shipped, evaluated, stamp_table).await
  }
}
