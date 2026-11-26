use std::{sync::Arc, thread};

use arcstr::ArcStr;
use oxc::span::Span;
use oxc_index::IndexVec;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rolldown_common::SourceMapGenMsg;
use rolldown_common::{
  EntryPoint, FlatOptions, HybridIndexVec, Module, ModuleId, ModuleIdx, ModuleTable,
  PreserveEntrySignatures, ResolvedId, RuntimeModuleBrief, ScanMode, StmtInfos, SymbolRefDb,
  dynamic_import_usage::DynamicImportExportsUsage, side_effects::HookSideEffects,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::{
  SharedOptions, SharedResolver,
  module_loader::{
    ModuleLoader,
    module_loader::{ModuleLoaderOutput, VisitState},
  },
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::scan_stage_cache::ScanStageCache,
};

type SourcemapChannel =
  (Option<std::sync::mpsc::Sender<SourceMapGenMsg>>, Option<thread::JoinHandle<()>>);

pub struct ScanStage<Fs: FileSystem + Clone + 'static> {
  options: SharedOptions,
  plugin_driver: SharedPluginDriver,
  fs: Fs,
  resolver: SharedResolver<Fs>,
}

#[derive(Debug)]
pub struct NormalizedScanStageOutput {
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  /// Per-module `StmtInfos` side table, parallel to `module_table.modules`.
  /// External modules get an empty `StmtInfos::new()` placeholder. Routed
  /// directly into `LinkStage.stmt_infos` instead of living on `EcmaView`.
  pub stmt_infos: IndexStmtInfos,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  // TODO: merge the preserve_entry_signatures_map in incremental build
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  pub tla_module_count: usize,
  pub tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
}

impl NormalizedScanStageOutput {
  /// Make a snapshot of the current ScanStage, skipping clone some fields that is immutable in
  /// following stage
  pub fn make_copy(&self) -> Self {
    Self {
      module_table: self.module_table.clone(),
      index_ecma_ast: {
        let iter = self.index_ecma_ast.raw.par_iter();

        let index_ecma_ast = iter
          .map(|ast| ast.as_ref().map(rolldown_ecmascript::EcmaAst::clone_with_another_arena))
          .collect::<Vec<_>>();
        IndexVec::from_vec(index_ecma_ast)
      },
      stmt_infos: self.stmt_infos.clone(),
      entry_points: self.entry_points.clone(),
      symbol_ref_db: self.symbol_ref_db.clone_without_scoping(),
      runtime: self.runtime.clone(),
      warnings: vec![],
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map.clone(),
      overrode_preserve_entry_signature_map: self.overrode_preserve_entry_signature_map.clone(),
      entry_point_to_reference_ids: self.entry_point_to_reference_ids.clone(),
      flat_options: self.flat_options,
      user_defined_entry_modules: self.user_defined_entry_modules.clone(),
      tla_module_count: self.tla_module_count,
      tla_keyword_span_map: self.tla_keyword_span_map.clone(),
    }
  }
}

impl TryFrom<ScanStageOutput> for NormalizedScanStageOutput {
  type Error = &'static str;

  fn try_from(value: ScanStageOutput) -> Result<Self, Self::Error> {
    let module_table = match value.module_table {
      HybridIndexVec::IndexVec(modules) => ModuleTable { modules },
      HybridIndexVec::Map(_) => return Err("module_table must be normalized to IndexVec first"),
    };

    let index_ecma_ast = match value.index_ecma_ast {
      HybridIndexVec::IndexVec(ast) => ast,
      HybridIndexVec::Map(_) => return Err("index_ecma_ast must be normalized to IndexVec first"),
    };

    let stmt_infos = match value.stmt_infos {
      HybridIndexVec::IndexVec(stmt_infos) => stmt_infos,
      HybridIndexVec::Map(_) => return Err("stmt_infos must be normalized to IndexVec first"),
    };

    Ok(Self {
      module_table,
      index_ecma_ast,
      stmt_infos,
      entry_points: value.entry_points,
      symbol_ref_db: value.symbol_ref_db,
      runtime: value.runtime,
      warnings: value.warnings,
      dynamic_import_exports_usage_map: value.dynamic_import_exports_usage_map,
      overrode_preserve_entry_signature_map: value.overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: value.entry_point_to_reference_ids,
      flat_options: value.flat_options,
      user_defined_entry_modules: value.user_defined_entry_modules,
      tla_module_count: value.tla_module_count,
      tla_keyword_span_map: value.tla_keyword_span_map,
    })
  }
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub module_table: HybridIndexVec<ModuleIdx, Module>,
  pub index_ecma_ast: HybridIndexVec<ModuleIdx, Option<EcmaAst>>,
  pub stmt_infos: HybridIndexVec<ModuleIdx, StmtInfos>,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  pub tla_module_count: usize,
  pub tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
}

impl<Fs: FileSystem + Clone + 'static> ScanStage<Fs> {
  pub fn new(
    options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: Fs,
    resolver: SharedResolver<Fs>,
  ) -> Self {
    Self { options, plugin_driver, fs, resolver }
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  pub async fn scan(
    &self,
    mode: ScanMode<ArcStr>,
    cache: &mut ScanStageCache,
  ) -> BuildResult<ScanStageOutput> {
    let fetch_mode = match mode {
      ScanMode::Full => ScanMode::Full,
      ScanMode::Partial(changed_ids) => {
        let resolved_ids = changed_resolved_ids(&changed_ids, cache);
        if let Err(err) = self.reset_changed_modules(&resolved_ids, &cache.pending_rescans).await {
          // `fetch_modules` has not taken the changed modules yet. Queue them for the next scan,
          // as a failed fetch does (`ModuleLoader::revert_partial_scan`).
          for resolved_id in resolved_ids {
            if cache.pending_rescans.iter().all(|queued| queued.id != resolved_id.id) {
              cache.pending_rescans.push(resolved_id);
            }
          }
          return Err(err);
        }
        ScanMode::Partial(resolved_ids)
      }
    };
    let (tx_clone, handler) = self.create_sourcemap_channel();

    let mut module_loader = ModuleLoader::new(
      self.fs.clone(),
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      cache,
      fetch_mode.is_full(),
      tx_clone,
    )?;

    // For `pluginContext.emitFile` with `type: chunk`, support it at buildStart hook.
    self
      .plugin_driver
      .file_emitter
      .set_context_load_modules_tx(Some(module_loader.shared_context.tx.clone()))?;

    self.plugin_driver.build_start(&self.options).await?;

    // For `await pluginContext.load`, if support it at buildStart hook, it could be caused stuck.
    self
      .plugin_driver
      .set_context_load_modules_tx(Some(module_loader.shared_context.tx.clone()))?;

    let module_loader_output = module_loader.fetch_modules(fetch_mode).await?;

    if let Some(handler) = handler {
      // The maps already sit in each module's sourcemap chain, in plugin
      // order. Joining only makes sure they are all generated.
      handler.join().expect("SourceMapGen: sourcemap worker thread panicked");
    }

    self.plugin_driver.file_emitter.set_context_load_modules_tx(None)?;

    self.plugin_driver.set_context_load_modules_tx(None)?;

    Ok(module_loader_output.into())
  }

  /// Resets the changed modules to the module options of the resolution that created them, before
  /// `buildStart` runs. `getModuleInfo` in `buildStart` then shows these start values, not the
  /// ones that the last fetch set, such as in the HMR stage of the same change. `fetch_modules`
  /// resets them again after `buildStart`.
  /// See internal-docs/module-meta-and-side-effects/design.md.
  async fn reset_changed_modules(
    &self,
    changed: &[ResolvedId],
    pending_rescans: &[ResolvedId],
  ) -> BuildResult<()> {
    let resolved_ids = || changed.iter().chain(pending_rescans);
    if resolved_ids().next().is_none() {
      return Ok(());
    }
    if let Some(reset_js_module_options) = &self.options.reset_js_module_options {
      let module_ids = resolved_ids().map(|resolved_id| resolved_id.id.to_string());
      reset_js_module_options.call(Some(module_ids.collect())).await?;
    }
    for resolved_id in resolved_ids() {
      self.plugin_driver.module_side_effects_overrides.remove(resolved_id.id.as_str());
      self.plugin_driver.set_module_info_side_effects(
        &resolved_id.id,
        resolved_id.side_effects.map(HookSideEffects::declared),
      );
    }
    Ok(())
  }

  fn create_sourcemap_channel(&self) -> SourcemapChannel {
    // Gated on the target, not the scheduler flavor: the drainer is an OS
    // thread, so it cannot starve the executor that feeds it, even on
    // `CurrentThread`. Both wasm builds generate maps inline; see
    // `can_spawn_os_threads` for why the threaded one does too.
    if !rolldown_utils::futures::can_spawn_os_threads() {
      return (None, None);
    }

    if self.options.is_sourcemap_enabled() {
      let (tx, rx) = std::sync::mpsc::channel::<SourceMapGenMsg>();
      // Dedicated OS thread, never a runtime `spawn_blocking`: a drainer that
      // took this consumer inline would block in `rx.recv()` forever, leaving
      // the module tasks that feed the channel unpolled -> deadlock.
      let handler = thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
          match msg {
            SourceMapGenMsg::MagicString(pending) => {
              // Generates the map now, off the JS thread, so it's ready at render time.
              pending.get();
            }
            SourceMapGenMsg::Terminate => {
              break;
            }
          }
        }
      });
      (Some(tx), Some(handler))
    } else {
      (None, None)
    }
  }
}

/// The resolutions that created the changed modules. A partial scan fetches each changed module
/// again from the resolution that created it, as an HMR update does, so no `resolveId` hook runs
/// again. A `resolveId` hook gets a specifier, not a resolved id, so calling it with the id of the
/// module can resolve to another module, for example for a virtual module. A changed file that
/// is not a module, such as a file that a plugin added with `addWatchFile`, is skipped.
///
/// A changed external module is not fetched, because it has no hooks to run. It is marked as
/// invalidated, so the first resolution of its id in this scan sets its kind again: a changed
/// file that was external can become a normal module.
fn changed_resolved_ids(changed_ids: &[ArcStr], cache: &mut ScanStageCache) -> Vec<ResolvedId> {
  let Some(snapshot) = cache.snapshot() else {
    return Vec::new();
  };
  let mut resolved_ids = Vec::with_capacity(changed_ids.len());
  let mut external_ids = Vec::new();
  for changed_id in changed_ids {
    // The dev engine passes file paths for edits, and module ids for lazy compilation.
    let module_id = ModuleId::new(changed_id.as_str());
    let idx = match cache.module_id_to_idx.get(&module_id) {
      Some(state) => state.idx(),
      None => {
        let path = std::path::Path::new(changed_id.as_str()).to_slash();
        let Some(&idx) = cache.module_idx_by_abs_path.get(&*path) else {
          continue;
        };
        idx
      }
    };
    match snapshot.module_table.modules.get(idx) {
      Some(Module::Normal(module)) => resolved_ids.push(module.originative_resolved_id.clone()),
      Some(Module::External(module)) => external_ids.push((module.id.clone(), idx)),
      None => {}
    }
  }
  for (module_id, idx) in external_ids {
    cache.module_id_to_idx.insert(module_id, VisitState::Invalidate(idx));
  }
  resolved_ids
}

impl From<ModuleLoaderOutput> for ScanStageOutput {
  fn from(module_loader_output: ModuleLoaderOutput) -> Self {
    let ModuleLoaderOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      stmt_infos,
      dynamic_import_exports_usage_map,
      new_added_modules_from_partial_scan: _,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      flat_options,
      user_defined_entry_modules,
      tla_module_count,
      tla_keyword_span_map,
    } = module_loader_output;
    ScanStageOutput {
      module_table,
      index_ecma_ast,
      stmt_infos,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      dynamic_import_exports_usage_map,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      flat_options,
      user_defined_entry_modules,
      tla_module_count,
      tla_keyword_span_map,
    }
  }
}
