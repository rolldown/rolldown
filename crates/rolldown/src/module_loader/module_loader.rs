use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::Arc;

use arcstr::ArcStr;
use futures::{FutureExt, StreamExt, stream::FuturesUnordered};
use itertools::{Either, Itertools};
use oxc::semantic::{ScopeId, Scoping};
use oxc::span::Span;
use oxc::transformer_plugins::ReplaceGlobalDefinesConfig;
use oxc_index::IndexVec;
use rolldown_common::dynamic_import_usage::DynamicImportExportsUsage;
use rolldown_common::{
  AddEntryModuleMsg, EcmaModuleAstUsage, EcmaRelated, EntryPoint, EntryPointKind, ExternalModule,
  ExternalModuleTaskResult, FlatOptions, HybridIndexVec, ImportKind, ImportRecordMeta,
  ImportedExports, ImporterRecord, Module, ModuleId, ModuleIdx, ModuleLoaderMsg, ModuleType,
  NormalModuleTaskResult, PreserveEntrySignatures, RUNTIME_MODULE_ID, ResolvedId,
  RuntimeModuleBrief, RuntimeModuleTaskResult, ScanMode, SourceMapGenMsg, StmtInfos, SymbolRefDb,
  SymbolRefDbForModule,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosableResolveError, SingleBuildResult, consolidate_diagnostics,
};
use rolldown_fs::FileSystem;
use rolldown_plugin::{ContextLoadProgress, SharedPluginDriver};
use rolldown_std_utils::discard_panic_payload;
use rolldown_utils::commondir;
use rolldown_utils::dashmap::FxDashSet;
use rolldown_utils::futures::try_spawn_detached;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rolldown_utils::rayon::{IntoParallelIterator, ParallelIterator};
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;
use tracing::Instrument;

use crate::module_loader::module_task::ModuleTaskOwner;
use crate::types::scan_stage_cache::ScanStageCache;
use crate::utils::load_entry_module::load_entry_module;
use crate::{SharedOptions, SharedResolver};

use super::external_module_task::ExternalModuleTask;
use super::module_task::ModuleTask;
use super::runtime_module_task::RuntimeModuleTask;
use super::task_context::{TaskContext, TaskContextMeta};

pub struct IntermediateNormalModules {
  pub modules: HybridIndexVec<ModuleIdx, Option<Module>>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  pub index_ecma_ast: HybridIndexVec<ModuleIdx, Option<EcmaAst>>,
  /// Per-module statement-info table collected as modules complete. Held here
  /// instead of on `EcmaView` so the link stage can carry it as a side
  /// `IndexVec<ModuleIdx, StmtInfos>` without a `mem::replace`.
  pub stmt_infos: HybridIndexVec<ModuleIdx, Option<StmtInfos>>,
}

impl IntermediateNormalModules {
  pub fn new(is_full_scan: bool, importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>) -> Self {
    Self {
      modules: if is_full_scan {
        HybridIndexVec::IndexVec(IndexVec::default())
      } else {
        HybridIndexVec::Map(FxHashMap::default())
      },
      importers,
      index_ecma_ast: if is_full_scan {
        HybridIndexVec::IndexVec(IndexVec::default())
      } else {
        HybridIndexVec::Map(FxHashMap::default())
      },
      stmt_infos: if is_full_scan {
        HybridIndexVec::IndexVec(IndexVec::default())
      } else {
        HybridIndexVec::Map(FxHashMap::default())
      },
    }
  }

  pub fn alloc_ecma_module_idx(&mut self) -> ModuleIdx {
    let id = self.modules.push(None);
    self.index_ecma_ast.push(None);
    self.stmt_infos.push(None);
    self.importers.push(Vec::new());
    id
  }

  pub fn alloc_ecma_module_idx_sparse(&mut self, i: ModuleIdx) -> ModuleIdx {
    self.modules.insert(i, None);
    self.index_ecma_ast.insert(i, None);
    self.stmt_infos.insert(i, None);
    if i >= self.importers.len() {
      self.importers.push(Vec::new());
    }
    i
  }
}

#[derive(Debug, Clone, Copy)]
pub enum VisitState {
  Seen(ModuleIdx),
  Invalidate(ModuleIdx),
}

impl VisitState {
  pub fn idx(self) -> ModuleIdx {
    match self {
      VisitState::Seen(idx) | VisitState::Invalidate(idx) => idx,
    }
  }
}

enum PendingEntry {
  UserDefined { index: usize, name: Option<ArcStr> },
  Emitted { index: usize, msg: Box<AddEntryModuleMsg> },
}

#[derive(Default)]
#[expect(clippy::type_complexity)]
struct ResolvedEntries {
  user_defined: Vec<Option<SingleBuildResult<(Option<ArcStr>, ResolvedId, ModuleIdx)>>>,
  emitted: Vec<Option<SingleBuildResult<(Box<AddEntryModuleMsg>, ModuleIdx)>>>,
}

/// A module whose index is reserved but whose task has not started, see
/// `ModuleLoader::deferred_asset_spawns`.
struct DeferredAssetSpawn {
  resolved_id: ResolvedId,
  owner: Option<ModuleTaskOwner>,
  create_js_module_options: bool,
}

pub struct ModuleLoader<'a, Fs: FileSystem + Clone + 'static> {
  pub shared_context: Arc<TaskContext<Fs>>,
  rx: futures::channel::mpsc::UnboundedReceiver<ModuleLoaderMsg>,
  remaining: u32,
  intermediate_normal_modules: IntermediateNormalModules,
  symbol_ref_db: SymbolRefDb,
  is_full_scan: bool,
  new_added_modules_from_partial_scan: FxIndexSet<ModuleIdx>,
  /// Ids inserted into `module_id_to_idx` by this partial scan;
  /// `revert_partial_scan` removes them again on failure.
  new_module_ids: Vec<ModuleId>,
  cache: &'a mut ScanStageCache,
  pub flat_options: FlatOptions,
  pub magic_string_tx: Option<std::sync::mpsc::Sender<SourceMapGenMsg>>,
  tla_module_count: usize,
  /// Centralized map from modules that contain top-level `await` to the span
  /// of the first TLA keyword. Stored here instead of on every `EcmaView`
  /// because top-level await is rarely used in real-world apps.
  tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
  /// This set contains the modules that this scan spawned as external. The set
  /// gives the kind of a module before its task finishes. See
  /// `is_external_module`.
  external_module_idxs: FxHashSet<ModuleIdx>,
  /// This list contains the errors of this scan. The message loop and helpers
  /// such as `try_spawn_new_task` add errors to it. `fetch_modules` returns the
  /// list after the scan.
  errors: Vec<BuildDiagnostic>,
  /// Whether an entry is still resolving.
  entries_pending: bool,
  /// Modules that an import record asserted as assets while an entry was still resolving. The
  /// first spawn of a module fixes whether it is an asset or an entry, and the target may be that
  /// entry, so the task starts when the entries are resolved, or earlier if something needs the
  /// module. `remaining` already counts them.
  deferred_asset_spawns: FxIndexMap<ModuleIdx, DeferredAssetSpawn>,
}

pub struct ModuleLoaderOutput {
  // Stored all modules
  pub module_table: HybridIndexVec<ModuleIdx, Module>,
  pub index_ecma_ast: HybridIndexVec<ModuleIdx, Option<EcmaAst>>,
  /// Side table of per-module `StmtInfos` (one slot per module index, with
  /// `StmtInfos::new()` placeholder for external modules). Threaded through
  /// `ScanStageOutput`/`NormalizedScanStageOutput` to the link stage instead
  /// of living on each `EcmaView`.
  pub stmt_infos: HybridIndexVec<ModuleIdx, StmtInfos>,
  pub symbol_ref_db: SymbolRefDb,
  // Entries that user defined + dynamic import entries
  pub entry_points: Vec<EntryPoint>,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  // Empty if it is a full scan
  pub new_added_modules_from_partial_scan: FxIndexSet<ModuleIdx>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  /// Record entry point and related reference ids generated by `this.emitFile`.
  /// Note, one entry point may related to multiple reference ids
  /// e.g. https://stackblitz.com/edit/rolldown-rolldown-starter-stackblitz-jqg7vnkw?file=rolldown.config.mjs,src%2Findex.js,package.json
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  pub tla_module_count: usize,
  pub tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
}

/// Covers a module task until its first poll. From then on the task's own `TaskMessageGuard`
/// reports a panic or a cancellation, so the two never report the same task twice.
struct ModuleTaskSupervisor {
  state: Option<(
    futures::channel::mpsc::UnboundedSender<ModuleLoaderMsg>,
    SharedPluginDriver,
    ModuleId,
  )>,
}

impl ModuleTaskSupervisor {
  fn disarm(&mut self) {
    self.state = None;
  }
}

impl Drop for ModuleTaskSupervisor {
  fn drop(&mut self) {
    let Some((tx, plugin_driver, module_id)) = self.state.take() else {
      return;
    };
    // A `this.load` of this module waits until the module is loaded. End that wait too.
    plugin_driver.mark_context_load_progress(module_id, ContextLoadProgress::Completed);
    let diagnostic = BuildDiagnostic::unhandleable_error(anyhow::anyhow!(
      "module loader task was dropped before it started"
    ));
    let _ = tx.unbounded_send(ModuleLoaderMsg::BuildErrors(vec![diagnostic].into_boxed_slice()));
  }
}

fn supervised_module_task<F>(
  future: F,
  tx: futures::channel::mpsc::UnboundedSender<ModuleLoaderMsg>,
  plugin_driver: SharedPluginDriver,
  module_id: ModuleId,
) -> impl std::future::Future<Output = ()> + Send + 'static
where
  F: std::future::Future<Output = ()> + Send + 'static,
{
  let mut supervisor = ModuleTaskSupervisor { state: Some((tx, plugin_driver, module_id)) };
  async move {
    supervisor.disarm();
    if let Err(payload) = AssertUnwindSafe(future).catch_unwind().await {
      discard_panic_payload(payload);
    }
  }
}

fn spawn_module_task<F>(
  future: F,
  tx: futures::channel::mpsc::UnboundedSender<ModuleLoaderMsg>,
  plugin_driver: SharedPluginDriver,
  module_id: ModuleId,
) where
  F: std::future::Future<Output = ()> + Send + 'static,
{
  // Module task state machines are tens of KB. Box once here so each hop of
  // the spawn chain moves a pointer instead of copying the state machine.
  let future: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> = Box::pin(future);
  let future = supervised_module_task(future, tx, plugin_driver, module_id);
  if let Err(future) = try_spawn_detached(future) {
    drop(future);
  }
}

impl<Fs: FileSystem + Clone> Drop for ModuleLoader<'_, Fs> {
  fn drop(&mut self) {
    self.cache.importers = std::mem::take(&mut self.intermediate_normal_modules.importers);
  }
}

impl<'a, Fs: FileSystem + Clone + 'static> ModuleLoader<'a, Fs> {
  pub fn new(
    fs: Fs,
    options: SharedOptions,
    resolver: SharedResolver<Fs>,
    plugin_driver: SharedPluginDriver,
    cache: &'a mut ScanStageCache,
    is_full_scan: bool,
    magic_string_tx: Option<std::sync::mpsc::Sender<SourceMapGenMsg>>,
  ) -> BuildResult<Self> {
    plugin_driver.module_side_effects_overrides.start_scan();
    if is_full_scan {
      // TODO: drop the cache in another thread
      // Since we may also run a full fetch in hmr mode when multiple files changed at the same time, we need to clear the cache
      // if we are in full scan mode
      std::mem::take(cache);
    }

    let flat_options = FlatOptions::from_shared_options(&options);
    let symbol_ref_db = SymbolRefDb::new();
    let meta = TaskContextMeta {
      replace_global_define_config: if options.define.is_empty() {
        None
      } else {
        ReplaceGlobalDefinesConfig::new(&options.define).map(Some).map_err(|errs| {
          errs
            .into_iter()
            .map(|err| BuildDiagnostic::invalid_define_config(err.message.to_string()))
            .collect::<Vec<BuildDiagnostic>>()
        })?
      },
    };

    // Unbounded as defense in depth. If any sender ends up driven from a sync
    // napi binding through `block_on`, a bounded channel could deadlock: the
    // send future would wait on capacity that only the consumer can free, but
    // the consumer may need the JS thread — pinned by `block_on` — to run
    // plugin hooks first. Keeping the channel unbounded removes that edge.
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let shared_context = Arc::new(TaskContext {
      options,
      tx,
      resolver,
      fs,
      plugin_driver,
      meta,
      late_entries: FxDashSet::default(),
    });

    let importers = std::mem::take(&mut cache.importers);
    let intermediate_normal_modules = IntermediateNormalModules::new(is_full_scan, importers);

    Ok(Self {
      rx,
      cache,
      remaining: 0,
      is_full_scan,
      shared_context,
      symbol_ref_db,
      intermediate_normal_modules,
      new_added_modules_from_partial_scan: FxIndexSet::default(),
      entries_pending: false,
      deferred_asset_spawns: FxIndexMap::default(),
      new_module_ids: Vec::new(),
      flat_options,
      magic_string_tx,
      tla_module_count: 0,
      tla_keyword_span_map: FxHashMap::default(),
      external_module_idxs: FxHashSet::default(),
      errors: Vec::new(),
    })
  }

  /// Restores the cache to its pre-scan state so the next build can stay
  /// incremental. The snapshot is never touched by a scan; everything else
  /// is re-derived from it or reset. Existing `module_id_to_idx` entries are
  /// self-healing (`Seen` -> `Invalidate` -> `Seen`), so only new keys need
  /// removal. A failed full scan needs nothing: its cache reset already
  /// forces a full retry.
  /// See internal-docs/bundler-data-lifecycle/implementation.md ("Cache integrity on a failed build").
  fn revert_partial_scan(
    &mut self,
    scanned_resolved_ids: Vec<ResolvedId>,
    barrel_state_backup: Option<rolldown_common::BarrelState>,
  ) {
    if self.is_full_scan {
      return;
    }
    for module_id in std::mem::take(&mut self.new_module_ids) {
      self.cache.module_id_to_idx.remove(&module_id);
    }
    // New indices are freed for reuse above; drop the `ModuleIdx`-keyed
    // transform dependencies recorded for them, or a later module reusing the
    // index would inherit them.
    for idx in &self.new_added_modules_from_partial_scan {
      self.shared_context.plugin_driver.transform_dependencies.remove(idx);
    }
    // Retry only files the graph still needs, judged on the freshest edge
    // state, where modules re-scanned by this failed scan already updated
    // their edges.
    let pending_rescans =
      self.cache.still_needed(&self.intermediate_normal_modules.importers, scanned_resolved_ids);
    self.intermediate_normal_modules.importers = self.cache.derive_importers_from_snapshot();
    self.cache.modules_with_changed_importers.clear();
    if let Some(backup) = barrel_state_backup {
      self.cache.barrel_state = backup;
    }
    self.cache.pending_rescans = pending_rescans;
  }

  /// Marks `idx` for importer-set re-derivation in `ScanStageCache::merge`;
  /// call it next to every mutation of `intermediate_normal_modules.importers`.
  /// No-op in full scan mode, which rebuilds every module's sets anyway.
  fn mark_module_importers_changed(&mut self, idx: ModuleIdx) {
    if !self.is_full_scan {
      self.cache.modules_with_changed_importers.insert(idx);
    }
  }

  /// This function returns `true` if this scan spawned the module at `idx`. A full
  /// scan spawns every module. A partial scan allocates a slot for each module that
  /// it spawns (changed and new modules).
  fn is_spawned_by_this_scan(&self, idx: ModuleIdx) -> bool {
    self.is_full_scan || self.intermediate_normal_modules.modules.try_get(idx).is_some()
  }

  /// This function returns `true` if the module at `idx` is external. The
  /// loader knows the kind when it spawns the task. Thus this function can
  /// return the kind before the task finishes.
  fn is_external_module(&self, idx: ModuleIdx) -> bool {
    // For a module that this scan spawned, the kind from this scan replaces the
    // kind in the snapshot. Thus a changed file that was external can now be a
    // normal module.
    if self.is_spawned_by_this_scan(idx) {
      self.external_module_idxs.contains(&idx)
    } else {
      // This scan did not spawn the module. The module is from a previous build,
      // and the snapshot has its kind.
      self.cache.get_snapshot().module_table.get(idx).is_some_and(Module::is_external)
    }
  }

  /// This function records an error if the two kinds are different:
  ///
  /// - `resolved_id` is external, and the module at `idx` is not external.
  /// - `resolved_id` is not external, and the module at `idx` is external.
  ///
  /// One id has one module, and the first resolution decides its kind. Without
  /// this error, the importer that loads first decides the kind (#11417).
  /// Rollup reports the same errors. See internal-docs/cache/implementation.md
  /// (invariant 7).
  fn check_external_conflict(
    &mut self,
    idx: ModuleIdx,
    resolved_id: &ResolvedId,
    owner: Option<&ModuleTaskOwner>,
  ) {
    let is_external = resolved_id.external.is_external();
    if is_external == self.is_external_module(idx) {
      return;
    }
    let module_id = resolved_id.id.as_arc_str().clone();
    let importer = owner.map(ModuleTaskOwner::to_external_conflict_importer);
    // A partial scan keeps the kind from the snapshot for a module that it did not
    // spawn. Then the error also suggests a full rebuild, because a full rebuild
    // takes the kind from the current imports. See the known gap in
    // internal-docs/cache/implementation.md (invariant 7).
    let is_kind_from_previous_build = !self.is_spawned_by_this_scan(idx);
    self.errors.push(if is_external {
      BuildDiagnostic::invalid_external_id(module_id, importer, is_kind_from_previous_build)
    } else {
      BuildDiagnostic::external_modules_cannot_be_transformed_to_modules(
        module_id,
        importer,
        is_kind_from_previous_build,
      )
    });
  }

  /// Lazy barrel must not skip any of an entry's re-exports — an entry has to
  /// preserve all of its exports. Request `All` from it: if it has already been
  /// loaded as a barrel, load its remaining (deferred) records now; otherwise
  /// record the request so they get loaded once it finishes loading.
  fn request_all_exports_for_entry(&mut self, idx: ModuleIdx) {
    if let Some(barrel_module_state) = self.cache.barrel_state.barrel_infos.get(&idx) {
      // If the module is already a barrel module, we need to load its remaining re-export import records
      if barrel_module_state.is_some() {
        self.process_barrel_import_record(&mut VecDeque::from_iter([(idx, ImportedExports::All)]));
      }
    } else {
      self.cache.barrel_state.requested_exports.insert(idx, ImportedExports::All);
    }
  }

  fn try_spawn_new_task(
    &mut self,
    mut resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<&ModuleType>,
  ) -> ModuleIdx {
    let mut is_new_module = true;
    let idx = match self.cache.module_id_to_idx.get(&resolved_id.id).copied() {
      Some(VisitState::Seen(idx)) => {
        self.check_external_conflict(idx, &resolved_id, owner.as_ref());
        if self.flat_options.is_lazy_barrel_enabled() && owner.is_none() {
          self.request_all_exports_for_entry(idx);
        }
        // An entry starts as a plain module. Anything else that reaches a deferred module, such
        // as `this.load()` or a plain import, may wait for it, so it starts as the asset.
        if is_user_defined_entry || assert_module_type != Some(&ModuleType::Asset) {
          self.start_deferred_asset_spawn(idx, !is_user_defined_entry);
        }
        return idx;
      }
      Some(VisitState::Invalidate(idx)) => {
        is_new_module = false;
        // An incremental rebuild resolves a changed file again without an importer, but the
        // module keeps the `moduleSideEffects` of the resolution that created it.
        if let Some(Module::Normal(module)) =
          self.cache.snapshot().and_then(|snapshot| snapshot.module_table.modules.get(idx))
        {
          resolved_id.side_effects = module.originative_resolved_id.side_effects;
        }
        // Full scan mode the idx will never be invalidated right?
        self.intermediate_normal_modules.alloc_ecma_module_idx_sparse(idx);
        self.cache.module_id_to_idx.insert(resolved_id.id.clone(), VisitState::Seen(idx));
        idx
      }
      None if !self.is_full_scan => {
        // This means some new module has been added in partial scan mode
        let len = self.cache.module_id_to_idx.len();
        let idx = self.intermediate_normal_modules.alloc_ecma_module_idx_sparse(len.into());
        self.new_added_modules_from_partial_scan.insert(idx);
        self.new_module_ids.push(resolved_id.id.clone());
        self.cache.module_id_to_idx.insert(resolved_id.id.clone(), VisitState::Seen(idx));
        idx
      }
      None => {
        let idx = self.intermediate_normal_modules.alloc_ecma_module_idx();
        self.cache.module_id_to_idx.insert(resolved_id.id.clone(), VisitState::Seen(idx));
        idx
      }
    };
    if self.flat_options.is_lazy_barrel_enabled() && owner.is_none() {
      self.request_all_exports_for_entry(idx);
    }
    // A full scan starts with no JS module options, so JS needs the call only for a module that a
    // JS resolution created. A partial scan may still hold options for the id.
    let create_js_module_options =
      is_new_module && (resolved_id.module_options_receipt.is_some() || !self.is_full_scan);
    let ctx = Arc::clone(&self.shared_context);
    let tx = self.shared_context.tx.clone();
    let plugin_driver = Arc::clone(&self.shared_context.plugin_driver);
    let module_id = resolved_id.id.clone();
    if resolved_id.external.is_external() {
      self.external_module_idxs.insert(idx);
      let task = ExternalModuleTask::new(ctx, idx, resolved_id, create_js_module_options);
      spawn_module_task(
        task.run().instrument(tracing::info_span!("external_module_task")),
        tx,
        plugin_driver,
        module_id,
      );
    } else if self.entries_pending
      && !is_user_defined_entry
      && owner.is_some()
      && assert_module_type == Some(&ModuleType::Asset)
    {
      self
        .deferred_asset_spawns
        .insert(idx, DeferredAssetSpawn { resolved_id, owner, create_js_module_options });
    } else {
      self.start_normal_module_task(
        idx,
        resolved_id,
        owner,
        is_user_defined_entry,
        assert_module_type,
        create_js_module_options,
      );
    }
    self.remaining += 1;
    idx
  }

  fn start_normal_module_task(
    &self,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<&ModuleType>,
    create_js_module_options: bool,
  ) {
    let module_id = resolved_id.id.clone();
    let task = ModuleTask::new(
      Arc::clone(&self.shared_context),
      idx,
      resolved_id,
      owner,
      is_user_defined_entry,
      assert_module_type.cloned(),
      self.flat_options,
      self.magic_string_tx.clone(),
      create_js_module_options,
    );
    spawn_module_task(
      task.run().instrument(tracing::info_span!("normal_module_task")),
      self.shared_context.tx.clone(),
      Arc::clone(&self.shared_context.plugin_driver),
      module_id,
    );
  }

  /// Starts the task of a deferred module, if `idx` is one. `as_asset` is false when the module
  /// became an entry.
  fn start_deferred_asset_spawn(&mut self, idx: ModuleIdx, as_asset: bool) {
    let Some(deferred) = self.deferred_asset_spawns.shift_remove(&idx) else { return };
    self.start_normal_module_task(
      idx,
      deferred.resolved_id,
      deferred.owner,
      !as_asset,
      as_asset.then_some(&ModuleType::Asset),
      deferred.create_js_module_options,
    );
  }

  /// For `fetch_modules` we need to support three scenarios:
  /// - Full scan mode in none watch mode, scan all modules from user defined entries.
  /// - Partial scan mode, scan the changed modules, it maybe none initial
  /// build in incremental watch mode
  /// - Full scan mode in watch mode, scan all modules from user defined entries, it maybe first
  /// time build in watch mode or edgecase in HMR(User update `node_modules` too much modules are
  /// updated at same time, patch them one by one is not efficient, so we do full scan directly)
  #[tracing::instrument(level = "debug", skip_all)]
  #[expect(clippy::too_many_lines)]
  pub async fn fetch_modules(
    &mut self,
    fetch_mode: ScanMode<ResolvedId>,
  ) -> BuildResult<ModuleLoaderOutput> {
    let mut all_warnings = vec![];

    // Barrel state is accumulated demand, not derivable from the snapshot;
    // back it up for `revert_partial_scan`. Disabled means empty and untouched.
    let barrel_state_backup = (!self.is_full_scan && self.flat_options.is_lazy_barrel_enabled())
      .then(|| self.cache.barrel_state.clone());

    // Initialize runtime module task if not yet started
    if let Entry::Vacant(e) = self.cache.module_id_to_idx.entry(RUNTIME_MODULE_ID) {
      let idx = self.intermediate_normal_modules.alloc_ecma_module_idx();
      let task = RuntimeModuleTask::new(idx, Arc::clone(&self.shared_context), self.flat_options);
      spawn_module_task(
        task.run().instrument(tracing::info_span!("runtime_module_task")),
        self.shared_context.tx.clone(),
        Arc::clone(&self.shared_context.plugin_driver),
        RUNTIME_MODULE_ID,
      );
      e.insert(VisitState::Seen(idx));
      self.remaining += 1;
    }

    // The message loop below runs while the entries resolve, so a `resolveId` hook of an entry
    // can wait for other modules, as in Rollup. All resolutions come from this one closure, so
    // `pending_entries` holds one future type.
    let ctx = Arc::clone(&self.shared_context);
    let resolve_entry = move |specifier: String, importer: Option<String>, entry: PendingEntry| {
      let ctx = Arc::clone(&ctx);
      async move {
        let result = load_entry_module(
          &ctx.options,
          &ctx.resolver,
          &ctx.plugin_driver,
          &specifier,
          importer.as_deref(),
        )
        .await;
        (entry, result)
      }
    };

    let mut pending_entries = FuturesUnordered::new();
    let mut resolved_entries = ResolvedEntries::default();
    if fetch_mode.is_full() {
      for input_item in &self.shared_context.options.input {
        let entry = PendingEntry::UserDefined {
          index: resolved_entries.user_defined.len(),
          name: input_item.name.as_ref().map(Into::into),
        };
        resolved_entries.user_defined.push(None);
        pending_entries.push(resolve_entry(input_item.import.clone(), None, entry));
      }
    }

    let entries_count = resolved_entries.user_defined.len() + /* runtime */ 1;
    self.intermediate_normal_modules.modules.reserve(entries_count);
    self.intermediate_normal_modules.index_ecma_ast.reserve(entries_count);

    // The modules of user-defined and emitted entries
    let mut user_defined_entry_ids = FxHashSet::with_capacity(entries_count);

    // If it is in partial scan mode, we need to invalidate the changed modules
    // and re-fetch them, do nothing in full scan mode. Files re-queued by a
    // previous aborted scan (`pending_rescans`) are retried alongside.
    let mut changed_resolved_ids = Vec::new();
    let mut seen_changed_ids = FxHashSet::default();
    for resolved_id in
      fetch_mode.iter().cloned().chain(std::mem::take(&mut self.cache.pending_rescans))
    {
      if seen_changed_ids.insert(resolved_id.id.clone()) {
        changed_resolved_ids.push(resolved_id);
      }
    }
    // Keep only the module options set when the changed modules were resolved, because their
    // importers do not resolve them again.
    for resolved_id in &changed_resolved_ids {
      self
        .shared_context
        .plugin_driver
        .module_side_effects_overrides
        .remove(resolved_id.id.as_str());
    }
    if !changed_resolved_ids.is_empty()
      && let Some(reset_js_module_options) = &self.shared_context.options.reset_js_module_options
    {
      let module_ids = changed_resolved_ids.iter().map(|resolved_id| resolved_id.id.to_string());
      if let Err(err) = reset_js_module_options.call(Some(module_ids.collect())).await {
        // The files were taken from `pending_rescans`, so queue them again.
        self.revert_partial_scan(changed_resolved_ids, barrel_state_backup);
        return Err(err.into());
      }
    }
    for resolved_id in changed_resolved_ids.iter().cloned() {
      self.shared_context.plugin_driver.invalidate_context_load_module(&resolved_id.id);
      if let Entry::Occupied(mut occ) = self.cache.module_id_to_idx.entry(resolved_id.id.clone()) {
        let idx = occ.get().idx();
        occ.insert(VisitState::Invalidate(idx));
        self.cache.barrel_state.barrel_infos.remove(&idx);
      }
      // User may update the entry module in incremental mode, so we need to make sure
      // if it is a user defined entry to avoid generate wrong asset file
      let is_user_defined_entry = self.cache.user_defined_entry.contains(&resolved_id.id);
      // Setting `Owner` to `None` is safe here since `Owner` is only used to emit
      // `Unloadable` diagnostics, and we know this module exists in the filesystem.
      // TODO: copy assert_module_type
      self.try_spawn_new_task(resolved_id, None, is_user_defined_entry, None);
    }

    let mut runtime_brief = None;
    let mut work_queue: VecDeque<(ModuleIdx, ImportedExports)> = VecDeque::new();
    let mut dynamic_import_entry_ids = FxHashMap::default();
    let mut dynamic_import_exports_usage_pairs = vec![];

    while self.remaining > 0 || !pending_entries.is_empty() {
      self.entries_pending = !pending_entries.is_empty();
      let event = if pending_entries.is_empty() {
        let Some(msg) = self.rx.next().await else { break };
        Either::Left(msg)
      } else {
        futures::select! {
          msg = self.rx.select_next_some() => Either::Left(msg),
          resolved_entry = pending_entries.select_next_some() => Either::Right(resolved_entry),
          complete => break,
        }
      };
      let msg = match event {
        Either::Left(msg) => msg,
        Either::Right((entry, result)) => {
          let result = result.map(|resolved_id| {
            // Another module can import an entry before the entry is resolved. The module
            // task has then started as a non-entry.
            if self.cache.module_id_to_idx.contains_key(&resolved_id.id) {
              self.shared_context.mark_entry(&resolved_id.id);
            }
            let idx = self.try_spawn_new_task(resolved_id.clone(), None, true, None);
            user_defined_entry_ids.insert(idx);
            (resolved_id, idx)
          });
          match entry {
            PendingEntry::UserDefined { index, name } => {
              resolved_entries.user_defined[index] =
                Some(result.map(|(resolved_id, idx)| (name, resolved_id, idx)));
            }
            PendingEntry::Emitted { index, msg } => {
              resolved_entries.emitted[index] = Some(result.map(|(_, idx)| (msg, idx)));
            }
          }
          if pending_entries.is_empty() {
            let idxs = self.deferred_asset_spawns.keys().copied().collect_vec();
            for idx in idxs {
              self.start_deferred_asset_spawn(idx, true);
            }
          }
          continue;
        }
      };
      match msg {
        ModuleLoaderMsg::NormalModuleDone(task_result) => {
          let NormalModuleTaskResult {
            mut module,
            mut barrel_info,
            ecma_related:
              EcmaRelated {
                ast,
                symbols,
                mut dynamic_import_rec_exports_usage,
                preserve_jsx,
                stmt_infos,
              },
            resolved_deps,
            raw_import_records,
            warnings,
            tla_keyword_span,
          } = *task_result;
          all_warnings.extend(warnings);

          let module_idx = module.idx();

          // remove importers from previous scan
          if !self.is_full_scan
            && let Some(previous_module) =
              self.cache.get_snapshot().module_table.modules.get(module_idx)
          {
            let resolved_deps = previous_module
              .import_records()
              .into_iter()
              .filter_map(|r| r.resolved_module)
              .collect::<Vec<_>>();
            for dep_idx in resolved_deps {
              self.intermediate_normal_modules.importers[dep_idx]
                .retain(|v| v.importer_idx != module_idx);
              self.mark_module_importers_changed(dep_idx);
            }
          }

          let normal_module = module.as_normal().unwrap();
          if normal_module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait) {
            self.tla_module_count += 1;
          }
          if let Some(span) = tla_keyword_span {
            self.tla_keyword_span_map.insert(module_idx, span);
          }
          let mut import_records = IndexVec::with_capacity(raw_import_records.len());

          let mut tracked_records = FxHashMap::default();
          let mut initialized_barrel_tracking =
            self.flat_options.is_lazy_barrel_enabled().then(|| {
              self.cache.barrel_state.initialize_barrel_tracking(
                normal_module,
                &raw_import_records,
                &mut barrel_info,
              )
            });

          for ((rec_idx, mut raw_rec), resolved_id) in
            raw_import_records.into_iter_enumerated().zip(resolved_deps)
          {
            if self.shared_context.options.experimental.vite_mode.unwrap_or_default()
              && resolved_id.id.raw_id().ends_with(".json")
            {
              raw_rec.meta.insert(ImportRecordMeta::JsonModule);
            }

            // Lazy barrel optimization: skip loading modules that are not needed yet.
            // - `imported_exports_per_record`: maps all import records to their required imported exports
            // - `initial_needed_records`: subset of records that need to be loaded initially
            if let Some((ref imported_exports_per_record, ref initial_needed_records)) =
              initialized_barrel_tracking
              && raw_rec.kind == ImportKind::Import
            {
              // Track remaining import records for later on-demand loading
              if imported_exports_per_record.contains_key(&rec_idx) {
                tracked_records.insert(rec_idx, (raw_rec.state.clone(), resolved_id.clone()));
              }
              // Skip records not in initial_needed_records - they may be loaded later if needed
              if !initial_needed_records.contains_key(&rec_idx) {
                import_records.push(raw_rec.into_resolved(None));
                continue;
              }
            }

            let is_external = resolved_id.external.is_external();
            let idx = self.try_spawn_new_task(
              resolved_id,
              Some(ModuleTaskOwner::new(normal_module, raw_rec.span)),
              false,
              raw_rec.asserted_module_type.as_ref(),
            );

            if let Some((_, ref mut initial_needed_records)) = initialized_barrel_tracking {
              let imported_exports = if raw_rec.kind == ImportKind::Import {
                initial_needed_records.remove(&rec_idx).expect(
                  "If initial_needed_records does not contain the record idx, it should have been skipped above"
                )
              } else {
                ImportedExports::All
              };
              work_queue.push_back((idx, imported_exports));
            }

            // Dynamic imported module will be considered as an entry
            self.intermediate_normal_modules.importers[idx].push(ImporterRecord {
              kind: raw_rec.kind,
              importer_path: module.id().clone(),
              importer_idx: module_idx,
            });
            self.mark_module_importers_changed(idx);

            // Defer usage merging - with only one consumer, keep fetch actions simple
            if let Some(usage) = dynamic_import_rec_exports_usage.remove(&rec_idx) {
              dynamic_import_exports_usage_pairs.push((idx, usage));
            }
            // This filter excludes external modules, so that each bit position in code
            // splitting stays equal to a `ChunkIdx`. `is_external` comes from this import
            // record, but it is the same as the kind of the module at `idx`. If the two
            // kinds are different, `try_spawn_new_task` records an error, and the build
            // stops before code splitting. User-defined entries and emitted entries are
            // never external:
            // - `load_entry_module()` rejects an external resolution with
            //   `entry_cannot_be_external`.
            // - `try_spawn_new_task` rejects an entry whose id is already an external module.
            if matches!(raw_rec.kind, ImportKind::DynamicImport) && !is_external {
              match dynamic_import_entry_ids.entry(idx) {
                Entry::Vacant(vac) => match raw_rec.dynamic_import_expr_info.as_ref() {
                  Some(info) => {
                    vac.insert(vec![(module_idx, info.stmt_info_idx, info.node_id, rec_idx)]);
                  }
                  None => {
                    vac.insert(vec![]);
                  }
                },
                Entry::Occupied(mut occ) => {
                  if let Some(info) = raw_rec.dynamic_import_expr_info.as_ref() {
                    occ.get_mut().push((module_idx, info.stmt_info_idx, info.node_id, rec_idx));
                  }
                }
              }
            }
            import_records.push(raw_rec.into_resolved(Some(idx)));
          }

          module.set_import_records(import_records);
          *self.intermediate_normal_modules.index_ecma_ast.get_mut(module_idx) = Some(ast);
          *self.intermediate_normal_modules.stmt_infos.get_mut(module_idx) = Some(stmt_infos);
          *self.intermediate_normal_modules.modules.get_mut(module_idx) = Some(module);

          if let Some((imported_exports_per_record, _)) = initialized_barrel_tracking {
            self.cache.barrel_state.barrel_infos.insert(
              module_idx,
              if tracked_records.is_empty() {
                None
              } else {
                barrel_info.map(|info| {
                  info.into_barrel_module_state(tracked_records, imported_exports_per_record)
                })
              },
            );
            self.process_barrel_import_record(&mut work_queue);
          }

          self.symbol_ref_db.store_local_db(module_idx, symbols);
          if preserve_jsx {
            self.symbol_ref_db.set_has_module_preserve_jsx();
          }
          self.remaining -= 1;
        }
        ModuleLoaderMsg::ExternalModuleDone(task_result) => {
          let ExternalModuleTaskResult {
            id,
            name,
            idx,
            identifier_name,
            side_effects,
            side_effects_without_option,
            need_renormalize_render_path,
            attributes,
          } = *task_result;

          self.symbol_ref_db.store_local_db(
            idx,
            SymbolRefDbForModule::new(Scoping::default(), idx, ScopeId::new(0)),
          );
          let symbol_ref = self.symbol_ref_db.create_facade_root_symbol_ref(idx, &identifier_name);
          let external_module = Module::External(Box::new(ExternalModule::new(
            idx,
            id,
            name,
            identifier_name,
            side_effects,
            side_effects_without_option,
            symbol_ref,
            need_renormalize_render_path,
            attributes,
          )));

          *self.intermediate_normal_modules.modules.get_mut(idx) = Some(external_module);
          self.remaining -= 1;
        }
        ModuleLoaderMsg::RuntimeNormalModuleDone(task_result) => {
          let RuntimeModuleTaskResult {
            local_symbol_ref_db,
            mut module,
            stmt_infos,
            runtime,
            ast,
            raw_import_records,
            resolved_deps,
            warnings,
          } = *task_result;
          all_warnings.extend(warnings);

          let mut import_records = IndexVec::with_capacity(raw_import_records.len());
          for (raw_rec, info) in raw_import_records.into_iter().zip(resolved_deps) {
            let idx =
              self.try_spawn_new_task(info, None, false, raw_rec.asserted_module_type.as_ref());
            self.intermediate_normal_modules.importers[idx].push(ImporterRecord {
              kind: raw_rec.kind,
              importer_path: module.id.clone(),
              importer_idx: module.idx,
            });
            self.mark_module_importers_changed(idx);
            import_records.push(raw_rec.into_resolved(Some(idx)));
          }
          module.import_records = import_records;

          *self.intermediate_normal_modules.modules.get_mut(runtime.id()) = Some(module.into());
          *self.intermediate_normal_modules.index_ecma_ast.get_mut(runtime.id()) = Some(ast);
          *self.intermediate_normal_modules.stmt_infos.get_mut(runtime.id()) = Some(stmt_infos);

          self.symbol_ref_db.store_local_db(runtime.id(), local_symbol_ref_db);
          self.remaining -= 1;

          self.errors.extend(runtime.validate_symbols(&rolldown_common::RUNTIME_HELPER_NAMES));
          runtime_brief = Some(runtime);
        }
        ModuleLoaderMsg::FetchModule(resolve_id) => {
          self.fetch_module(*resolve_id);
        }
        ModuleLoaderMsg::AddEntryModule(msg) => {
          let specifier = msg.chunk.id.clone();
          let importer = msg.chunk.importer.clone();
          let entry = PendingEntry::Emitted { index: resolved_entries.emitted.len(), msg };
          resolved_entries.emitted.push(None);
          pending_entries.push(resolve_entry(specifier, importer, entry));
        }
        ModuleLoaderMsg::BuildErrors(e) => {
          self.errors.extend(e);
          self.remaining -= 1;
        }
      }
    }

    // The entries are in input and emit order, whatever order their resolution finished in.
    let mut errors = vec![];
    let mut entry_points = FxIndexSet::default();
    let mut user_defined_entries = Vec::with_capacity(resolved_entries.user_defined.len());
    for result in resolved_entries.user_defined.into_iter().flatten() {
      match result {
        Ok((name, resolved_id, idx)) => {
          entry_points.insert(EntryPoint {
            idx,
            name: name.clone(),
            kind: EntryPointKind::UserDefined,
            file_name: None,
            related_stmt_infos: vec![],
          });
          user_defined_entries.push((name, resolved_id));
        }
        Err(err) => errors.push(err),
      }
    }

    let mut extra_entry_points = vec![];
    let mut entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>> = FxHashMap::default();
    let mut overrode_preserve_entry_signature_map = FxHashMap::default();
    for result in resolved_entries.emitted.into_iter().flatten() {
      match result {
        Ok((msg, idx)) => {
          let data = &msg.chunk;
          if let Some(preserve_entry_signatures) = data.preserve_entry_signatures {
            overrode_preserve_entry_signature_map.insert(idx, preserve_entry_signatures);
          }
          let entry = EntryPoint {
            name: data.name.clone(),
            idx,
            kind: EntryPointKind::EmittedUserDefined,
            file_name: data.file_name.clone(),
            related_stmt_infos: vec![],
          };
          entry_point_to_reference_ids
            .entry(entry.clone())
            .or_default()
            .push(msg.reference_id.clone());
          extra_entry_points.push(entry);
        }
        Err(err) => errors.push(err),
      }
    }

    if self.is_full_scan && self.shared_context.options.experimental.is_incremental_build_enabled()
    {
      self
        .cache
        .user_defined_entry
        .extend(user_defined_entries.iter().map(|(_, resolved_id)| resolved_id.id.clone()));
    }

    self.render_new_external_paths(&user_defined_entries, &extra_entry_points);

    errors.append(&mut self.errors);
    if !errors.is_empty() {
      // Enrich UNRESOLVED_IMPORT errors with import chain. Must run before
      // the revert below, which erases the failed scan's modules from the
      // lookup structures the trace walks.
      for error in &mut errors {
        if let Some(resolve_error) = error.downcast_mut::<DiagnosableResolveError>() {
          let chain = self
            .trace_import_chain_from_modules(&resolve_error.importer_id, &user_defined_entry_ids);
          if !chain.is_empty() {
            resolve_error.import_chain = Some(chain);
          }
        }
      }
      self.revert_partial_scan(changed_resolved_ids, barrel_state_backup);
      return Err(consolidate_diagnostics(errors).into());
    }
    if let Some(tx) = self.magic_string_tx.as_ref() {
      tx.send(SourceMapGenMsg::Terminate).expect(
        "SourceMapGen: failed to send Terminate message - sourcemap worker thread died unexpectedly"
      );
    }

    let dynamic_import_exports_usage_map = dynamic_import_exports_usage_pairs.into_iter().fold(
      FxHashMap::default(),
      |mut acc, (idx, usage)| {
        match acc.entry(idx) {
          Entry::Vacant(vac) => {
            vac.insert(usage);
          }
          Entry::Occupied(mut occ) => {
            occ.get_mut().merge(usage);
          }
        }
        acc
      },
    );

    let mut idx_of_module_info_need_update = vec![];
    let is_dense_index_vec = self.intermediate_normal_modules.modules.is_index_vec();

    let modules_iter = std::mem::take(&mut self.intermediate_normal_modules.modules)
      .into_iter_enumerated()
      .into_iter()
      .map(|(idx, module)| {
        let mut module = module.expect("Module tasks did't complete as expected");
        if let Some(module) = module.as_normal_mut() {
          // Note: (Compat to rollup)
          // The `dynamic_importers/importers` should be added after `module_parsed` hook.
          let importers = &self.intermediate_normal_modules.importers[idx];
          module.ecma_view.rebuild_importer_sets(importers);
          if !importers.is_empty() {
            idx_of_module_info_need_update.push(idx);
          }
        }
        (idx, module)
      });

    let module_table = if is_dense_index_vec {
      let vec = modules_iter.map(|(_, module)| module).collect();
      HybridIndexVec::IndexVec(IndexVec::from_vec(vec))
    } else {
      let map = modules_iter.collect::<FxHashMap<_, _>>();
      HybridIndexVec::Map(map)
    };

    // Build the side `stmt_infos` table parallel to `module_table`. Slots that
    // never received a `Some(stmt_infos)` (external modules, or normal modules
    // not produced by tasks) get a fresh empty `StmtInfos` placeholder.
    let stmt_infos_iter = std::mem::take(&mut self.intermediate_normal_modules.stmt_infos)
      .into_iter_enumerated()
      .into_iter()
      .map(|(idx, stmt_infos)| (idx, stmt_infos.unwrap_or_else(StmtInfos::new)));
    let stmt_infos = if is_dense_index_vec {
      let vec = stmt_infos_iter.map(|(_, s)| s).collect();
      HybridIndexVec::IndexVec(IndexVec::from_vec(vec))
    } else {
      let map = stmt_infos_iter.collect::<FxHashMap<_, _>>();
      HybridIndexVec::Map(map)
    };

    // An import, `this.load` or an emitted chunk (#5030) can load an entry module before its
    // entry resolves. Then its module info says that it is not an entry.
    idx_of_module_info_need_update.extend(user_defined_entry_ids.iter().copied());
    idx_of_module_info_need_update.into_par_iter().for_each(|idx| {
      // A partial scan only holds re-scanned modules. A chunk emitted for an
      // unchanged module (e.g. by `buildStart` on the rebuild after a lazy
      // compile) is read from the last successful build's snapshot.
      let module = match module_table.try_get(idx) {
        Some(module) => Some(module),
        None => self.cache.snapshot().and_then(|snapshot| snapshot.module_table.modules.get(idx)),
      };
      let Some(module) = module.and_then(Module::as_normal) else {
        return;
      };
      let plugin_driver = &self.shared_context.plugin_driver;
      plugin_driver.set_module_info(
        &module.id,
        Arc::new(module.to_module_info(
          None,
          user_defined_entry_ids.contains(&module.idx),
          plugin_driver.module_info_side_effects(&module.id),
        )),
      );
    });

    // When a module is both dynamically imported and a user-defined or emitted entry, the
    // entry takes priority (it has user-specified name, fileName, preserveSignature)
    for (idx, related_stmt_infos) in dynamic_import_entry_ids {
      if !user_defined_entry_ids.contains(&idx) {
        entry_points.insert(EntryPoint {
          name: None,
          idx,
          kind: EntryPointKind::DynamicImport,
          file_name: None,
          related_stmt_infos,
        });
      }
    }

    entry_points.extend(extra_entry_points);
    if entry_points.is_empty() && self.is_full_scan {
      Err(BuildDiagnostic::invalid_option(rolldown_error::InvalidOptionType::NoEntryPoint))?;
    }

    let entry_points = entry_points.into_iter().collect_vec();
    // if it is in incremental mode, we skip the runtime module, since it is always there
    // so use a dummy runtime_brief as a placeholder
    let runtime = if self.is_full_scan {
      tracing::debug!(
        scan_mode = if fetch_mode.is_full() { "full" } else { "partial" },
        changed_resolved_ids = %changed_resolved_ids.iter().map(|id| &id.id).join(", "),
        "module fetch completed"
      );
      runtime_brief.expect("Failed to find runtime module. This should not happen")
    } else {
      RuntimeModuleBrief::dummy()
    };

    Ok(ModuleLoaderOutput {
      runtime,
      entry_points,
      module_table,
      warnings: all_warnings,
      dynamic_import_exports_usage_map,
      overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids,
      symbol_ref_db: std::mem::take(&mut self.symbol_ref_db),
      index_ecma_ast: std::mem::take(&mut self.intermediate_normal_modules.index_ecma_ast),
      stmt_infos,
      new_added_modules_from_partial_scan: std::mem::take(
        &mut self.new_added_modules_from_partial_scan,
      ),
      flat_options: self.flat_options,
      user_defined_entry_modules: user_defined_entry_ids,
      tla_module_count: self.tla_module_count,
      tla_keyword_span_map: std::mem::take(&mut self.tla_keyword_span_map),
    })
  }

  /// Renders each path external this scan added relative to the common directory of the entries.
  /// A full scan computes that directory here, after the scan, because emitted chunks join the
  /// entries during it.
  fn render_new_external_paths(
    &mut self,
    user_defined_entries: &[(Option<ArcStr>, ResolvedId)],
    extra_entry_points: &[EntryPoint],
  ) {
    if self.is_full_scan {
      let modules = &self.intermediate_normal_modules.modules;
      let emitted_entry_ids = extra_entry_points.iter().filter_map(|entry| {
        let id = modules.try_get(entry.idx)?.as_ref()?.id();
        id.is_path().then(|| id.raw_id())
      });
      let common_dir = commondir::common_dir(
        user_defined_entries
          .iter()
          .map(|(_, resolved_id)| resolved_id.id.raw_id())
          .chain(emitted_entry_ids),
      );
      self.cache.user_defined_entry_common_dir = common_dir.map(Arc::from);
    }
    let cwd = self.shared_context.options.cwd.as_path();
    let common_dir = self.cache.user_defined_entry_common_dir.as_deref().unwrap_or(cwd);
    for &idx in &self.external_module_idxs {
      if let Some(Module::External(module)) = self.intermediate_normal_modules.modules.get_mut(idx)
        && module.need_renormalize_render_path
      {
        module.name = Path::new(module.id.raw_id()).relative(common_dir).to_slash().into();
      }
    }
  }

  fn fetch_module(&mut self, resolve_id: ResolvedId) {
    let module_id = resolve_id.id.clone();
    let idx = self.try_spawn_new_task(resolve_id, None, false, None);
    // `this.load` waits for a load signal, and only a normal module task sends it.
    // A `this.load` of an id that is already external is a conflict, and
    // `try_spawn_new_task` records it as an error. The code below ends the wait, so
    // the build stops with that error and does not hang. No normal module task runs
    // for an external module in this scan, so the wait gets no second signal.
    if self.is_external_module(idx) {
      self
        .shared_context
        .plugin_driver
        .mark_context_load_progress(module_id, ContextLoadProgress::Completed);
    }
  }

  /// Traces the import chain from a module back to an entry point.
  /// Returns a list of module paths from the given module to an entry point.
  /// This version works directly with intermediate modules before the module table is built.
  fn trace_import_chain_from_modules(
    &self,
    importer_id: &str,
    user_defined_entry_ids: &FxHashSet<ModuleIdx>,
  ) -> Vec<String> {
    let importer_module_id = ModuleId::new(importer_id);
    let Some(visit_state) = self.cache.module_id_to_idx.get(&importer_module_id) else {
      return vec![];
    };
    let start_idx = visit_state.idx();

    let mut chain = Vec::new();
    let mut visited = FxHashSet::default();
    let mut current = Some(start_idx);

    while let Some(idx) = current {
      if visited.contains(&idx) {
        break;
      }
      visited.insert(idx);

      // A partial scan only holds re-scanned modules, so read unchanged ones
      // from the last successful build's snapshot. An empty slot stays skipped
      // to keep the failed module out of its own chain.
      let module_opt = match self.intermediate_normal_modules.modules.try_get(idx) {
        Some(slot) => slot.as_ref(),
        None => self.cache.snapshot().and_then(|snapshot| snapshot.module_table.modules.get(idx)),
      };
      if let Some(normal) = module_opt.and_then(Module::as_normal) {
        chain.push(normal.id.to_string());
      }

      if user_defined_entry_ids.contains(&idx)
        || self
          .cache
          .snapshot()
          .is_some_and(|snapshot| snapshot.user_defined_entry_modules.contains(&idx))
      {
        break;
      }

      current = self
        .intermediate_normal_modules
        .importers
        .get(idx)
        .and_then(|importers| importers.first().map(|rec| rec.importer_idx));
    }
    chain
  }

  fn process_barrel_import_record(
    &mut self,
    work_queue: &mut VecDeque<(ModuleIdx, ImportedExports)>,
  ) {
    while let Some((idx, imported_exports)) = work_queue.pop_front() {
      // No entry in `barrel_infos` means this module hasn't been loaded and processed yet.
      // Record it for now and process later when loading is complete.
      let Some(barrel_module_state) = self.cache.barrel_state.barrel_infos.get_mut(&idx) else {
        match self.cache.barrel_state.requested_exports.entry(idx) {
          Entry::Occupied(mut occ) => {
            imported_exports.subtract_and_merge_into(occ.get_mut());
          }
          Entry::Vacant(vac) => {
            vac.insert(imported_exports);
          }
        }
        continue;
      };

      // Not a barrel module, clean up and skip
      let Some(barrel_module_state) = barrel_module_state else {
        self.cache.barrel_state.requested_exports.remove(&idx);
        continue;
      };

      // No tracked records, skip
      if barrel_module_state.tracked_records.is_empty() {
        continue;
      }

      // Calculate new exports to process (subtract already processed)
      let new_exports = match self.cache.barrel_state.requested_exports.entry(idx) {
        Entry::Occupied(mut occ) => {
          let Some(diff) = imported_exports.subtract_and_merge_into(occ.get_mut()) else {
            continue;
          };
          diff
        }
        Entry::Vacant(vac) => {
          vac.insert(imported_exports.clone());
          imported_exports
        }
      };

      let mut tracked_records = std::mem::take(&mut barrel_module_state.tracked_records);
      let mut remaining_imported_specifiers =
        std::mem::take(&mut barrel_module_state.remaining_imported_specifiers);

      // Below repeats the post-module-load processing logic, but only for the remaining `tracked_records`.
      // Additionally, we check whether each record in `tracked_records` still needs to be retained.
      let mut needed_records = barrel_module_state
        .info
        .take_needed_records(&new_exports, &mut remaining_imported_specifiers);

      tracked_records.retain(|&rec_idx, (import_record_state, resolved_id)| {
        let Some(needed_record) = needed_records.remove(&rec_idx) else {
          return true;
        };

        let mut is_module_from_cache_snapshot = false;
        let barrel_normal_module = match &self.intermediate_normal_modules.modules {
          HybridIndexVec::IndexVec(modules) => modules[idx]
            .as_ref()
            .expect("Barrel module should exists in full build")
            .as_normal()
            .unwrap(),
          HybridIndexVec::Map(modules) => {
            if let Some(module) = modules.get(&idx) {
              module
                .as_ref()
                .expect("Barrel module should exists in partial build")
                .as_normal()
                .unwrap()
            } else {
              is_module_from_cache_snapshot = true;
              self
                .cache
                .get_snapshot()
                .module_table
                .get(idx)
                .expect("Barrel module should exist in cache snapshot in partial scan mode")
                .as_normal()
                .unwrap()
            }
          }
        };

        let target_idx = match barrel_normal_module.import_records[rec_idx].resolved_module {
          Some(existing_idx) => existing_idx,
          None => {
            let importer_record = ImporterRecord {
              kind: barrel_normal_module.import_records[rec_idx].kind,
              importer_path: barrel_normal_module.id.clone(),
              importer_idx: barrel_normal_module.idx,
            };
            let new_idx = self.try_spawn_new_task(
              resolved_id.clone(),
              Some(ModuleTaskOwner::new(barrel_normal_module, import_record_state.span)),
              false,
              import_record_state.asserted_module_type.as_ref(),
            );
            self.intermediate_normal_modules.importers[new_idx].push(importer_record);
            self.mark_module_importers_changed(new_idx);
            // Update resolved module in either cache snapshot or intermediate modules
            if is_module_from_cache_snapshot {
              self
                .cache
                .barrel_state
                .resolved_barrel_modules
                .entry(idx)
                .or_default()
                .push((rec_idx, new_idx));
            } else {
              self
                .intermediate_normal_modules
                .modules
                .get_mut(idx)
                .as_mut()
                .expect("barrel module should exist")
                .as_normal_mut()
                .unwrap()
                .import_records[rec_idx]
                .resolved_module = Some(new_idx);
            }
            new_idx
          }
        };
        let keep_tracking = match self.cache.barrel_state.barrel_infos.get(&target_idx) {
          Some(Some(s)) => target_idx == idx || !s.tracked_records.is_empty(),
          Some(None) => false,
          None => true,
        };
        work_queue.push_back((target_idx, needed_record));
        if keep_tracking {
          remaining_imported_specifiers.contains_key(&rec_idx)
        } else {
          remaining_imported_specifiers.remove(&rec_idx);
          false
        }
      });

      let barrel_module_state =
        self.cache.barrel_state.barrel_infos.get_mut(&idx).unwrap().as_mut().unwrap();
      barrel_module_state.tracked_records = tracked_records;
      barrel_module_state.remaining_imported_specifiers = remaining_imported_specifiers;
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use futures::StreamExt;
  use rolldown_common::{
    BundleMode, BundlerOptions, FlatOptions, ModuleId, ModuleLoaderMsg, ResolvedId,
  };
  use rolldown_fs::OsFileSystem;

  use crate::{
    BundleFactory, BundleFactoryOptions,
    module_loader::task_context::{TaskContext, TaskContextMeta},
  };

  use super::{ExternalModuleTask, FxDashSet, ModuleTask, supervised_module_task};

  /// A module task that finishes after its build was dropped must not panic when it sends.
  #[tokio::test]
  async fn module_tasks_ignore_a_dropped_receiver() {
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      bundler_options: BundlerOptions::default(),
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    let bundle = factory.create_bundle(BundleMode::FullBuild, None).expect("create bundle");
    // Drop the receiver at once, as a dropped build does.
    let (tx, _) = futures::channel::mpsc::unbounded();
    let ctx: Arc<TaskContext<OsFileSystem>> = Arc::new(TaskContext {
      options: bundle.options,
      tx,
      resolver: bundle.resolver,
      fs: bundle.fs,
      plugin_driver: bundle.plugin_driver,
      meta: TaskContextMeta { replace_global_define_config: None },
      late_entries: FxDashSet::default(),
    });

    let normal = ResolvedId { id: ModuleId::new_empty("empty"), ..Default::default() };
    let flat_options = FlatOptions::from_shared_options(&ctx.options);
    ModuleTask::new(
      Arc::clone(&ctx),
      0.into(),
      normal,
      None,
      false,
      None,
      flat_options,
      None,
      false,
    )
    .run()
    .await;

    let external =
      ResolvedId { id: ModuleId::new("external"), external: true.into(), ..Default::default() };
    ExternalModuleTask::new(Arc::clone(&ctx), 1.into(), external, false).run().await;
  }

  fn test_plugin_driver() -> rolldown_plugin::SharedPluginDriver {
    let mut factory = BundleFactory::new(BundleFactoryOptions {
      bundler_options: BundlerOptions::default(),
      disable_tracing_setup: true,
      ..Default::default()
    })
    .expect("create bundle factory");
    factory.create_bundle(BundleMode::FullBuild, None).expect("create bundle").plugin_driver
  }

  #[test]
  fn module_task_dropped_before_it_starts_reports_exactly_one_error() {
    use std::task::{Context, Poll, Waker};

    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let plugin_driver = test_plugin_driver();
    let module_id = ModuleId::new("dropped-before-start");

    // Register a `this.load()` waiter before the unstarted task drops.
    let mut waiter = Box::pin(plugin_driver.wait_for_module_load_completion(&module_id, true));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(
      matches!(waiter.as_mut().poll(&mut cx), Poll::Pending),
      "waiter must be pending before task failure"
    );

    let future = supervised_module_task(
      std::future::pending(),
      tx,
      Arc::clone(&plugin_driver),
      module_id.clone(),
    );
    drop(future);

    assert!(
      matches!(waiter.as_mut().poll(&mut cx), Poll::Ready(())),
      "a module task dropped before its first poll must unblock `this.load()` waiters"
    );

    let message = futures::executor::block_on(async { rx.next().await })
      .expect("a module task dropped before its first poll must retire loader accounting");
    match message {
      ModuleLoaderMsg::BuildErrors(errors) => assert_eq!(errors.len(), 1),
      _ => panic!("a module task dropped before its first poll must become BuildErrors"),
    }
    assert!(rx.try_recv().is_err(), "the drop must produce exactly one completion error");
  }

  #[test]
  fn started_module_task_leaves_reporting_to_its_own_guard() {
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let plugin_driver = test_plugin_driver();
    futures::executor::block_on(supervised_module_task(
      async {
        panic!("module task probe");
      },
      tx,
      plugin_driver,
      ModuleId::new("started-task"),
    ));
    assert!(rx.try_recv().is_err(), "a started task reports through `TaskMessageGuard` only");
  }
}
