use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use arcstr::ArcStr;
use futures::future::join_all;
use itertools::Itertools;
use oxc::semantic::NodeId;
use oxc::semantic::{ScopeId, Scoping};
use oxc::span::Span;
use oxc::transformer_plugins::ReplaceGlobalDefinesConfig;
use oxc_index::IndexVec;
use rolldown_common::dynamic_import_usage::DynamicImportExportsUsage;
use rolldown_common::{
  EcmaModuleAstUsage, EcmaRelated, EntryPoint, EntryPointKind, ExportsKind, ExternalModule,
  ExternalModuleTaskResult, FlatOptions, HybridIndexVec, ImportKind, ImportRecordIdx,
  ImportRecordMeta, ImportedExports, ImporterRecord, Module, ModuleId, ModuleIdx, ModuleLoaderMsg,
  ModuleType, NormalModuleTaskResult, PreserveEntrySignatures, RUNTIME_MODULE_ID, ResolvedId,
  RuntimeModuleBrief, RuntimeModuleTaskResult, ScanMode, SourceMapGenMsg, StmtInfoIdx, StmtInfos,
  SymbolRef, SymbolRefDb, SymbolRefDbForModule,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::{
  AmbiguousExternalNamespaceModule, BuildDiagnostic, BuildResult, DiagnosableResolveError,
  DiagnosticOptions, consolidate_diagnostics,
};
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::indexmap::FxIndexSet;
use rolldown_utils::rayon::{IntoParallelIterator, ParallelIterator};
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::Instrument;

use crate::module_loader::module_task::ModuleTaskOwner;
use crate::stages::link_stage::exports_namespace_as_then;
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

#[derive(Clone)]
enum ScannedExportResolution {
  Unknown,
  Missing,
  /// The module whose export re-entered the resolution, as the link stage reports it.
  Circular(ModuleIdx, String),
  Bindings(Vec<(SymbolRef, ModuleIdx, Span)>),
}

impl VisitState {
  pub fn idx(self) -> ModuleIdx {
    match self {
      VisitState::Seen(idx) | VisitState::Invalidate(idx) => idx,
    }
  }
}

pub struct ModuleLoader<'a, Fs: FileSystem + Clone + 'static> {
  tasks: tokio::task::JoinSet<()>,
  /// The module that each unfinished task loads, to name it if the task fails.
  task_modules: FxHashMap<tokio::task::Id, ModuleId>,
  pub shared_context: Arc<TaskContext<Fs>>,
  rx: tokio::sync::mpsc::UnboundedReceiver<ModuleLoaderMsg>,
  remaining: u32,
  intermediate_normal_modules: IntermediateNormalModules,
  symbol_ref_db: SymbolRefDb,
  is_full_scan: bool,
  new_added_modules_from_partial_scan: FxIndexSet<ModuleIdx>,
  /// Ids inserted into `module_id_to_idx` by this partial scan;
  /// `revert_partial_scan` removes them again on failure.
  new_module_ids: Vec<ModuleId>,
  /// The cached modules that this partial scan loads again, which it found invalidated;
  /// `revert_partial_scan` invalidates them again on failure.
  reloaded_modules: Vec<(ModuleId, ModuleIdx)>,
  cache: &'a mut ScanStageCache,
  pub flat_options: FlatOptions,
  pub magic_string_tx: Option<std::sync::mpsc::Sender<SourceMapGenMsg>>,
  tla_module_count: usize,
  /// Centralized map from modules that contain top-level `await` to the span
  /// of the first TLA keyword. Stored here instead of on every `EcmaView`
  /// because top-level await is rarely used in real-world apps.
  tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
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

impl<Fs: FileSystem + Clone> Drop for ModuleLoader<'_, Fs> {
  fn drop(&mut self) {
    self.tasks.abort_all();
    self.cache.importers = std::mem::take(&mut self.intermediate_normal_modules.importers);
    let mut kept = std::mem::take(
      &mut *self
        .shared_context
        .deferred_resolution_targets
        .lock()
        .expect("resolution failure state poisoned"),
    );
    kept.retain(|id, _| !self.cache.module_id_to_idx.contains_key(id));
    self.cache.deferred_resolution_diagnostics.extend(kept);
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
    if is_full_scan {
      // TODO: drop the cache in another thread
      // Since we may also run a full fetch in hmr mode when multiple files changed at the same time, we need to clear the cache
      // if we are in full scan mode
      cache.reset_for_full_scan();
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
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    // Only an incremental build keeps the ASTs and scope trees that its scans build.
    let compact_module_arenas =
      options.experimental.is_incremental_build_enabled() && !cache.keep_unused_capacity;
    let deferred_resolution_targets =
      std::sync::Mutex::new(std::mem::take(&mut cache.deferred_resolution_diagnostics));
    let shared_context = Arc::new(TaskContext {
      options,
      tx,
      resolver,
      fs,
      plugin_driver,
      meta,
      deferred_parse_errors: Default::default(),
      deferred_resolution_targets,
      compact_module_arenas,
    });

    let importers = std::mem::take(&mut cache.importers);
    let intermediate_normal_modules = IntermediateNormalModules::new(is_full_scan, importers);

    Ok(Self {
      tasks: tokio::task::JoinSet::new(),
      task_modules: FxHashMap::default(),
      rx,
      cache,
      remaining: 0,
      is_full_scan,
      shared_context,
      symbol_ref_db,
      intermediate_normal_modules,
      new_added_modules_from_partial_scan: FxIndexSet::default(),
      new_module_ids: Vec::new(),
      reloaded_modules: Vec::new(),
      flat_options,
      magic_string_tx,
      tla_module_count: 0,
      tla_keyword_span_map: FxHashMap::default(),
    })
  }

  /// Restores the cache to its pre-scan state so the next build can stay
  /// incremental. The snapshot is never touched by a scan; everything else
  /// is re-derived from it or reset. New keys of `module_id_to_idx` are
  /// removed, and the cached modules that the scan loaded again are
  /// invalidated again. A failed full scan needs nothing: its cache reset
  /// already forces a full retry.
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
    // The cache keeps the earlier version of a module that the scan loaded again, which no
    // longer matches its file, so whichever later scan reaches the module, also through an
    // import of it rather than as a changed file, must load it again (#690).
    for (module_id, idx) in std::mem::take(&mut self.reloaded_modules) {
      self.cache.module_id_to_idx.insert(module_id, VisitState::Invalidate(idx));
    }
    // New indices are freed for reuse above; drop the `ModuleIdx`-keyed
    // transform dependencies recorded for them, or a later module reusing the
    // index would inherit them.
    for idx in &self.new_added_modules_from_partial_scan {
      self.shared_context.plugin_driver.transform_dependencies.remove(idx);
    }
    // Retry only files the graph still needs: entries, or files something
    // still imports (judged on the freshest edge state, where modules
    // re-scanned by this failed scan already updated their edges). A broken
    // file whose last import was removed must not fail every later build;
    // a fresh build of the same tree would succeed without it.
    let pending_rescans = scanned_resolved_ids
      .into_iter()
      .filter(|resolved_id| {
        if self.cache.user_defined_entry.contains(&resolved_id.id) {
          return true;
        }
        let Some(state) = self.cache.module_id_to_idx.get(&resolved_id.id) else {
          return false;
        };
        let idx = state.idx();
        self.cache.get_snapshot().user_defined_entry_modules.contains(&idx)
          || self
            .intermediate_normal_modules
            .importers
            .get(idx)
            .is_some_and(|records| !records.is_empty())
      })
      .collect();
    self.intermediate_normal_modules.importers = self.cache.derive_importers_from_snapshot();
    self.cache.modules_with_changed_importers.clear();
    if let Some(backup) = barrel_state_backup {
      self.cache.barrel_state = backup;
    }
    self.cache.pending_rescans = pending_rescans;
  }

  /// The modules whose tasks failed in this partial scan, whose slots stay empty.
  fn failed_tasks(&self) -> Vec<ModuleIdx> {
    match &self.intermediate_normal_modules.modules {
      HybridIndexVec::Map(modules) => {
        modules.iter().filter(|(_, module)| module.is_none()).map(|(idx, _)| *idx).collect()
      }
      HybridIndexVec::IndexVec(_) => Vec::new(),
    }
  }

  /// Whether the graph that a failed partial scan leaves reaches none of the `failed` modules:
  /// no entry, emitted entry or the runtime reaches them through the importers that the scan
  /// leaves, of any kind. A fresh build of the revision may then succeed, since it never loads
  /// them, as when an update drops a module's last import and breaks the module (#690). A module
  /// that a plugin loads with `this.load` counts as unreached, as does one that the scan added; a
  /// caller that builds the revision anew finds out.
  fn unreached(
    &self,
    failed: impl IntoIterator<Item = ModuleIdx>,
    entries: &FxHashSet<ModuleIdx>,
  ) -> bool {
    if self.is_full_scan {
      return false;
    }
    let mut stack = failed.into_iter().collect_vec();
    if stack.is_empty() {
      return false;
    }
    let mut roots = entries.clone();
    roots.extend(self.cache.get_snapshot().user_defined_entry_modules.iter().copied());
    roots.extend(
      self
        .cache
        .user_defined_entry
        .iter()
        .chain([&RUNTIME_MODULE_ID])
        .filter_map(|id| self.cache.module_id_to_idx.get(id).map(|state| state.idx())),
    );
    let mut seen = stack.iter().copied().collect::<FxHashSet<_>>();
    while let Some(idx) = stack.pop() {
      if roots.contains(&idx) {
        return false;
      }
      for importer in &self.intermediate_normal_modules.importers[idx] {
        if seen.insert(importer.importer_idx) {
          stack.push(importer.importer_idx);
        }
      }
    }
    true
  }

  /// Marks `idx` for importer-set re-derivation in `ScanStageCache::merge`;
  /// call it next to every mutation of `intermediate_normal_modules.importers`.
  /// No-op in full scan mode, which rebuilds every module's sets anyway.
  fn mark_module_importers_changed(&mut self, idx: ModuleIdx) {
    if !self.is_full_scan {
      self.cache.modules_with_changed_importers.insert(idx);
    }
  }

  /// Lazy barrel must not skip any of an entry's re-exports — an entry has to
  /// preserve all of its exports. Request `All` from it: if it has already been
  /// loaded as a barrel, load its remaining (deferred) records now; otherwise
  /// record the request so they get loaded once it finishes loading.
  #[expect(clippy::rc_buffer)]
  fn request_all_exports_for_entry(
    &mut self,
    idx: ModuleIdx,
    user_defined_entries: &Arc<Vec<(Option<ArcStr>, ResolvedId)>>,
  ) {
    if let Some(barrel_module_state) = self.cache.barrel_state.barrel_infos.get(&idx) {
      // If the module is already a barrel module, we need to load its remaining re-export import records
      if barrel_module_state.is_some() {
        self.process_barrel_import_record(
          &mut VecDeque::from_iter([(idx, ImportedExports::All, Demand::Exports)]),
          user_defined_entries,
        );
      }
    } else {
      self.cache.barrel_state.requested_exports.insert(idx, ImportedExports::All);
    }
  }

  #[expect(clippy::rc_buffer)]
  fn try_spawn_new_task(
    &mut self,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<&ModuleType>,
    user_defined_entries: &Arc<Vec<(Option<ArcStr>, ResolvedId)>>,
  ) -> ModuleIdx {
    let idx = match self.cache.module_id_to_idx.get(&resolved_id.id).copied() {
      Some(VisitState::Seen(idx)) => {
        if self.flat_options.is_lazy_barrel_enabled() && is_user_defined_entry {
          self.request_all_exports_for_entry(idx, user_defined_entries);
        }
        return idx;
      }
      Some(VisitState::Invalidate(idx)) => {
        // Full scan mode the idx will never be invalidated right?
        self.intermediate_normal_modules.alloc_ecma_module_idx_sparse(idx);
        self.cache.module_id_to_idx.insert(resolved_id.id.clone(), VisitState::Seen(idx));
        // A barrel loaded again tracks its records anew, as a changed module does.
        self.cache.barrel_state.barrel_infos.remove(&idx);
        self.reloaded_modules.push((resolved_id.id.clone(), idx));
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
    if self.flat_options.is_lazy_barrel_enabled() && is_user_defined_entry {
      self.request_all_exports_for_entry(idx, user_defined_entries);
    }
    let ctx = Arc::clone(&self.shared_context);
    let module_id = resolved_id.id.clone();
    let spawned = if resolved_id.external.is_external() {
      let task = ExternalModuleTask::new(ctx, idx, resolved_id, Arc::clone(user_defined_entries));
      self.tasks.spawn(task.run().instrument(tracing::info_span!("external_module_task")))
    } else {
      let task = ModuleTask::new(
        ctx,
        idx,
        resolved_id,
        owner,
        is_user_defined_entry,
        assert_module_type.cloned(),
        self.flat_options,
        self.magic_string_tx.clone(),
      );
      self.tasks.spawn(task.run().instrument(tracing::info_span!("normal_module_task")))
    };
    self.task_modules.insert(spawned.id(), module_id);
    self.remaining += 1;
    idx
  }

  /// The build error for a task that ended without sending its result.
  fn task_failure(&mut self, error: tokio::task::JoinError) -> BuildDiagnostic {
    let module = self
      .task_modules
      .remove(&error.id())
      .map_or_else(|| "an unknown module".to_string(), |id| format!("{:?}", id.as_str()));
    let failure = match error.try_into_panic() {
      Ok(payload) => {
        let message = payload
          .downcast_ref::<&str>()
          .copied()
          .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
          .unwrap_or("Box<dyn Any>");
        format!("module task panicked while loading {module}: {message}")
      }
      Err(_) => format!("module task was cancelled while loading {module}"),
    };
    BuildDiagnostic::unhandleable_error(anyhow::anyhow!(failure))
  }

  /// Aborts the unfinished tasks and waits for them to end, so that none outlives a failed
  /// scan, and returns the build errors of those that panicked meanwhile.
  async fn stop_tasks(&mut self) -> Vec<BuildDiagnostic> {
    self.tasks.abort_all();
    let mut failures = vec![];
    while let Some(joined) = self.tasks.join_next_with_id().await {
      if let Err(error) = joined
        && error.is_panic()
      {
        failures.push(self.task_failure(error));
      }
    }
    self.task_modules.clear();
    failures
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
    let mut errors = vec![];
    let mut all_warnings = vec![];
    // How many of `errors` module tasks reported, for `unreached`.
    let mut module_task_errors = 0;
    self.cache.last_scan_failures_unreached = false;

    // Barrel state is accumulated demand, not derivable from the snapshot;
    // back it up for `revert_partial_scan`. Disabled means empty and untouched.
    let barrel_state_backup = (!self.is_full_scan && self.flat_options.is_lazy_barrel_enabled())
      .then(|| self.cache.barrel_state.clone());

    // Initialize runtime module task if not yet started
    if let Entry::Vacant(e) = self.cache.module_id_to_idx.entry(RUNTIME_MODULE_ID) {
      let idx = self.intermediate_normal_modules.alloc_ecma_module_idx();
      let task = RuntimeModuleTask::new(idx, Arc::clone(&self.shared_context), self.flat_options);
      let spawned =
        self.tasks.spawn(task.run().instrument(tracing::info_span!("runtime_module_task")));
      self.task_modules.insert(spawned.id(), RUNTIME_MODULE_ID);
      e.insert(VisitState::Seen(idx));
      self.remaining += 1;
    }

    let user_defined_entries = Arc::new(match fetch_mode {
      ScanMode::Full => self.resolve_user_defined_entries().await?,
      ScanMode::Partial(_) => vec![],
    });

    let entries_count = user_defined_entries.len() + /* runtime */ 1;
    self.intermediate_normal_modules.modules.reserve(entries_count);
    self.intermediate_normal_modules.index_ecma_ast.reserve(entries_count);

    // Store the already consider as entry module
    let mut entry_points = FxIndexSet::default();
    let mut user_defined_entry_ids = FxHashSet::with_capacity(user_defined_entries.len());
    for (name, resolved_id) in user_defined_entries.iter().cloned() {
      let idx = self.try_spawn_new_task(resolved_id, None, true, None, &user_defined_entries);
      user_defined_entry_ids.insert(idx);
      entry_points.insert(EntryPoint {
        idx,
        name,
        kind: EntryPointKind::UserDefined,
        file_name: None,
        related_stmt_infos: vec![],
      });
    }

    if self.is_full_scan && self.shared_context.options.experimental.is_incremental_build_enabled()
    {
      self
        .cache
        .user_defined_entry
        .extend(user_defined_entries.iter().map(|(_, resolved_id)| resolved_id.id.clone()));
    }

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
    for resolved_id in changed_resolved_ids.iter().cloned() {
      self.shared_context.plugin_driver.invalidate_context_load_module(&resolved_id.id);
      if let Entry::Occupied(mut occ) = self.cache.module_id_to_idx.entry(resolved_id.id.clone()) {
        let idx = occ.get().idx();
        occ.insert(VisitState::Invalidate(idx));
        self.cache.barrel_state.barrel_infos.remove(&idx);
        let url_deps: Vec<_> = self
          .cache
          .get_snapshot()
          .module_table
          .modules
          .get(idx)
          .and_then(Module::as_normal)
          .map(|module| {
            module
              .import_records
              .iter()
              .filter(|r| r.kind == ImportKind::NewUrl)
              .filter_map(|rec| {
                let dep_idx = rec.resolved_module?;
                let dep_mod = self.cache.get_snapshot().module_table.modules.get(dep_idx)?.as_normal()?;
                Some((dep_mod.id.clone(), dep_idx))
              })
              .collect()
          })
          .unwrap_or_default();
        for (dep_id, dep_idx) in url_deps {
          self.shared_context.plugin_driver.invalidate_context_load_module(&dep_id);
          self.cache.module_id_to_idx.insert(dep_id, VisitState::Invalidate(dep_idx));
        }
      }
      // User may update the entry module in incremental mode, so we need to make sure
      // if it is a user defined entry to avoid generate wrong asset file
      let is_user_defined_entry = self.cache.user_defined_entry.contains(&resolved_id.id);
      // Setting `Owner` to `None` is safe here since `Owner` is only used to emit
      // `Unloadable` diagnostics, and we know this module exists in the filesystem.
      // TODO: copy assert_module_type
      self.try_spawn_new_task(
        resolved_id,
        None,
        is_user_defined_entry,
        None,
        &user_defined_entries,
      );
    }

    // The exports that a module's importers request from it, and what else they need of it.
    let mut work_queue: VecDeque<(ModuleIdx, ImportedExports, Demand)> = VecDeque::new();
    let mut dynamic_import_entry_ids: FxHashMap<
      ModuleIdx,
      Vec<(ModuleIdx, StmtInfoIdx, NodeId, ImportRecordIdx)>,
    > = FxHashMap::default();

    let mut dynamic_import_exports_usage_pairs = vec![];
    let mut extra_entry_points = vec![];
    let mut entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>> = FxHashMap::default();

    let mut runtime_brief = None;
    let mut overrode_preserve_entry_signature_map = FxHashMap::default();

    while self.remaining > 0 {
      let msg = tokio::select! {
        msg = self.rx.recv() => msg,
        Some(joined) = self.tasks.join_next_with_id() => {
          match joined {
            Ok((id, ())) => {
              self.task_modules.remove(&id);
            }
            // A task that panicked never sends its result, so waiting for it would never end.
            Err(error) => {
              errors.push(self.task_failure(error));
              errors.extend(self.stop_tasks().await);
              break;
            }
          }
          continue;
        }
      };
      let Some(msg) = msg else {
        break;
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
          let commonjs = normal_module.exports_kind == ExportsKind::CommonJs;
          if normal_module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait) {
            self.tla_module_count += 1;
          }
          if let Some(span) = tla_keyword_span {
            self.tla_keyword_span_map.insert(module_idx, span);
          }
          let mut import_records = IndexVec::with_capacity(raw_import_records.len());

          let mut tracked_records = FxHashMap::default();
          // The modules that the module's `export *` records load now.
          let mut star_targets = Vec::new();
          let mut initialized_barrel_tracking =
            self.flat_options.is_lazy_barrel_enabled().then(|| {
              self.cache.barrel_state.initialize_barrel_tracking(
                normal_module,
                &raw_import_records,
                &mut barrel_info,
              )
            });
          if let Some((remaining, needed)) = initialized_barrel_tracking.as_mut()
            && self.intermediate_normal_modules.importers.get(module_idx).is_some_and(|importers| {
              importers.iter().any(|importer| importer.kind == ImportKind::DynamicImport)
            })
          {
            let whole = barrel_info.as_ref().is_some_and(|info| !info.star.is_empty())
              || exports_callable_then(normal_module);
            take_body_records(
              |rec_idx| !whole && is_reexport(raw_import_records[rec_idx].meta),
              remaining,
              needed,
            );
          }

          for ((rec_idx, mut raw_rec), resolved_id) in
            raw_import_records.into_iter_enumerated().zip(resolved_deps)
          {
            if self.shared_context.options.experimental.vite_mode.unwrap_or_default()
              && resolved_id.id.as_str().ends_with(".json")
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
              &user_defined_entries,
            );
            if raw_rec.meta.contains(ImportRecordMeta::IsExportStar) {
              star_targets.push(idx);
            }

            if let Some((_, ref mut initial_needed_records)) = initialized_barrel_tracking {
              let imported_exports = match raw_rec.kind {
                ImportKind::Import => initial_needed_records.remove(&rec_idx).expect(
                  "If initial_needed_records does not contain the record idx, it should have been skipped above"
                ),
                // An `import()` builds only the names that the scanner saw it read, which a read
                // that the scanner can't follow turns into every name (#502, #510); see
                // `take_body_records` for what else it builds. An inlined one, without code
                // splitting, keeps the module's whole namespace.
                ImportKind::DynamicImport
                  if !self.shared_context.options.code_splitting.is_disabled() =>
                {
                  match dynamic_import_rec_exports_usage.get(&rec_idx) {
                    Some(DynamicImportExportsUsage::Partial(names)) => {
                      ImportedExports::Partial(names.clone())
                    }
                    Some(DynamicImportExportsUsage::Single(name)) => {
                      ImportedExports::Partial(FxHashSet::from_iter([name.clone()]))
                    }
                    Some(DynamicImportExportsUsage::Complete) | None => ImportedExports::All,
                  }
                }
                _ => ImportedExports::All,
              };
              work_queue.push_back((
                idx,
                imported_exports,
                if raw_rec.kind == ImportKind::DynamicImport {
                  Demand::Run
                } else {
                  Demand::Exports
                },
              ));
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
            // External modules are excluded here to preserve the bit-position-to-ChunkIdx
            // invariant in code splitting. User-defined and emitted entries are already
            // guaranteed non-external by `load_entry_module()` which rejects them with
            // `entry_cannot_be_external`.
            if matches!(raw_rec.kind, ImportKind::DynamicImport)
              && !is_external
              && !user_defined_entry_ids.contains(&idx)
            {
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
            // A CommonJS module gives the modules whose `export *` reaches it dynamic exports, and so
            // does a star of this module that reaches one that the scan loaded before.
            if commonjs || star_targets.iter().any(|&target| self.dynamic_exports(target)) {
              self.load_bodies_above(module_idx, &mut work_queue);
            }
            self.process_barrel_import_record(&mut work_queue, &user_defined_entries);
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
            need_renormalize_render_path,
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
            symbol_ref,
            need_renormalize_render_path,
          )));

          *self.intermediate_normal_modules.modules.get_mut(idx) = Some(external_module);
          // So does an external module.
          if self.flat_options.is_lazy_barrel_enabled() {
            self.load_bodies_above(idx, &mut work_queue);
            self.process_barrel_import_record(&mut work_queue, &user_defined_entries);
          }
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
            let idx = self.try_spawn_new_task(
              info,
              None,
              false,
              raw_rec.asserted_module_type.as_ref(),
              &user_defined_entries,
            );
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

          errors.extend(runtime.validate_symbols(&rolldown_common::RUNTIME_HELPER_NAMES));
          runtime_brief = Some(runtime);
        }
        ModuleLoaderMsg::FetchModule(resolve_id) => {
          self.try_spawn_new_task(*resolve_id, None, false, None, &user_defined_entries);
        }
        ModuleLoaderMsg::AddEntryModule(msg) => {
          let data = msg.chunk;
          let result = load_entry_module(
            &self.shared_context.resolver,
            &self.shared_context.plugin_driver,
            &data.id,
            data.importer.as_deref(),
          )
          .await;

          let module_idx = match result {
            Ok(resolved_id) => {
              self.try_spawn_new_task(resolved_id, None, true, None, &user_defined_entries)
            }
            Err(e) => {
              errors.push(e);
              continue;
            }
          };

          if let Some(preserve_entry_signatures) = data.preserve_entry_signatures {
            overrode_preserve_entry_signature_map.insert(module_idx, preserve_entry_signatures);
          }

          user_defined_entry_ids.insert(module_idx);

          let entry = EntryPoint {
            name: data.name.clone(),
            idx: module_idx,
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
        ModuleLoaderMsg::BuildErrors(e) => {
          module_task_errors += e.len();
          errors.extend(e);
          self.remaining -= 1;
        }
      }
    }

    self.cache.last_scan_failures_unreached = !errors.is_empty()
      && errors.len() == module_task_errors
      && self.unreached(self.failed_tasks(), &user_defined_entry_ids);

    if errors.is_empty() {
      // Keep the recovery scanner's large future out of every caller's build future.
      match Box::pin(self.isolate_dynamic_parse_errors(&user_defined_entry_ids)).await {
        Ok(replaced) => {
          dynamic_import_entry_ids.retain(|_, references| {
            let was_empty = references.is_empty();
            references.retain(|(importer, _, _, _)| !replaced.contains(importer));
            was_empty || !references.is_empty()
          });
        }
        Err(failures) => errors.extend(failures.into_vec()),
      }
    }
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

      let errors = consolidate_diagnostics(errors);
      return Err(errors.into());
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

    // Some module was not treated as an entry, but was emitted by `this.emitFile` during
    // processing, those module info also need to be updated
    // see https://github.com/rolldown/rolldown/issues/5030 as an example
    idx_of_module_info_need_update.extend(extra_entry_points.iter().map(|item| item.idx));
    idx_of_module_info_need_update.into_par_iter().for_each(|idx| {
      let module = module_table.get(idx);
      let Some(module) = module.as_normal() else {
        return;
      };
      self.shared_context.plugin_driver.set_module_info(
        &module.id,
        Arc::new(module.to_module_info(None, user_defined_entry_ids.contains(&module.idx))),
      );
    });

    // Collect module indices from emitted entries to filter dynamic imports
    // When a module is both dynamically imported AND emitted via this.emitFile,
    // the emitted entry takes priority (it has user-specified name, fileName, preserveSignature)
    let emitted_entry_indices: FxHashSet<ModuleIdx> =
      extra_entry_points.iter().map(|e| e.idx).collect();

    for (idx, related_stmt_infos) in dynamic_import_entry_ids {
      if !emitted_entry_indices.contains(&idx) {
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

  fn scanned_normal_module(&self, idx: ModuleIdx) -> Option<&rolldown_common::NormalModule> {
    let scanned = match &self.intermediate_normal_modules.modules {
      HybridIndexVec::IndexVec(modules) => modules.get(idx).and_then(Option::as_ref),
      HybridIndexVec::Map(modules) => modules.get(&idx).and_then(Option::as_ref),
    };
    scanned
      .or_else(|| self.cache.snapshot()?.module_table.modules.get(idx))
      .and_then(Module::as_normal)
  }

  /// The module that record `rec_idx` of scanned module `module` loads. What this scan resolves
  /// for a barrel that the cache snapshot holds stays in `resolved_barrel_modules` until the merge
  /// writes it into the barrel's records.
  fn resolved_module(
    &self,
    module: &rolldown_common::NormalModule,
    rec_idx: ImportRecordIdx,
  ) -> Option<ModuleIdx> {
    module.import_records[rec_idx].resolved_module.or_else(|| {
      self
        .cache
        .barrel_state
        .resolved_barrel_modules
        .get(&module.idx)?
        .iter()
        .find_map(|&(resolved, target)| (resolved == rec_idx).then_some(target))
    })
  }

  /// The records of scanned module `module` that load a module, in source order, with the module
  /// that each loads; see [`Self::resolved_module`].
  fn resolved_records(
    &self,
    module: &rolldown_common::NormalModule,
  ) -> Vec<(ImportRecordIdx, ModuleIdx)> {
    let mut records: Vec<_> = module
      .import_records
      .iter_enumerated()
      .filter_map(|(rec_idx, rec)| Some((rec_idx, rec.resolved_module?)))
      .collect();
    if let Some(resolved_later) = self.cache.barrel_state.resolved_barrel_modules.get(&module.idx) {
      records.extend(resolved_later);
      records.sort_unstable_by_key(|&(rec_idx, _)| rec_idx);
    }
    records
  }

  fn resolve_scanned_export(
    &self,
    idx: ModuleIdx,
    name: &str,
    loading_failures: &FxHashMap<ModuleIdx, Vec<BuildDiagnostic>>,
    stack: &mut FxHashSet<(ModuleIdx, String)>,
  ) -> ScannedExportResolution {
    use ScannedExportResolution::{Bindings, Circular, Missing, Unknown};
    let Some(module) = self.scanned_normal_module(idx) else {
      return Unknown;
    };
    if loading_failures.contains_key(&idx)
      || module.exports_kind != rolldown_common::ExportsKind::Esm
      || module.meta.has_lazy_export()
    {
      return Unknown;
    }
    let key = (idx, name.to_owned());
    if !stack.insert(key.clone()) {
      return Circular(idx, key.1);
    }
    let result = if let Some(export) = module.named_exports.get(name) {
      if export.came_from_commonjs {
        Unknown
      } else if let Some(import) = module.named_imports.get(&export.referenced) {
        match self.resolved_module(module, import.record_idx) {
          Some(target) => match &import.imported {
            rolldown_common::Specifier::Literal(imported) => {
              self.resolve_scanned_export(target, imported, loading_failures, stack)
            }
            rolldown_common::Specifier::Star => {
              self.scanned_normal_module(target).map_or(Unknown, |target| {
                Bindings(vec![(target.namespace_object_ref, idx, export.span)])
              })
            }
          },
          None => Unknown,
        }
      } else {
        Bindings(vec![(export.referenced, idx, export.span)])
      }
    } else if name == "default" || !module.has_star_export() {
      Missing
    } else {
      let mut bindings = Vec::new();
      let mut unknown = false;
      let mut visit_target = |target: Option<ModuleIdx>| {
        let Some(target) = target else {
          unknown = true;
          return;
        };
        match self.resolve_scanned_export(target, name, loading_failures, stack) {
          Unknown => unknown = true,
          // A star cycle contributes no binding. An explicit indirect cycle
          // remains an error when its own import/re-export is checked.
          Circular(..) | Missing => {}
          Bindings(resolved) => {
            for binding in resolved {
              if !bindings.iter().any(|(symbol, _, _)| *symbol == binding.0) {
                bindings.push(binding);
              }
            }
          }
        }
      };
      if !self.cache.barrel_state.resolved_barrel_modules.contains_key(&module.idx) {
        for rec in module
          .import_records
          .iter()
          .filter(|rec| rec.meta.contains(ImportRecordMeta::IsExportStar))
        {
          visit_target(rec.resolved_module);
        }
      } else {
        let targets = self.star_targets(module);
        let star_count = module
          .import_records
          .iter()
          .filter(|rec| rec.meta.contains(ImportRecordMeta::IsExportStar))
          .count();
        if targets.len() < star_count {
          visit_target(None);
        }
        for target in targets {
          visit_target(Some(target));
        }
      }
      if unknown {
        Unknown
      } else if bindings.is_empty() {
        Missing
      } else {
        Bindings(bindings)
      }
    };
    stack.remove(&key);
    result
  }

  /// Orders terminal failures as HTML's "find the first parse error" walks a module graph:
  /// each module before its static requests, requests in source order, entries in load order,
  /// and every loading failure before any link failure. The first diagnostic is then the one a
  /// browser reports, rather than one in hash order.
  fn in_first_parse_error_order(
    &self,
    entries: &FxHashSet<ModuleIdx>,
    loading: &FxHashSet<ModuleIdx>,
    mut failures: FxHashMap<ModuleIdx, Vec<BuildDiagnostic>>,
  ) -> Vec<BuildDiagnostic> {
    let mut roots: Vec<ModuleIdx> = entries
      .iter()
      .copied()
      .chain(
        self
          .cache
          .user_defined_entry
          .iter()
          .filter_map(|id| self.cache.module_id_to_idx.get(id).map(|state| state.idx())),
      )
      .collect();
    roots.sort_unstable();
    roots.dedup();
    let mut walk = Vec::new();
    let mut seen = FxHashSet::default();
    for root in roots {
      let mut stack = vec![root];
      while let Some(idx) = stack.pop() {
        if !seen.insert(idx) {
          continue;
        }
        walk.push(idx);
        // A module that failed to load has no requests of its own to visit.
        if loading.contains(&idx) {
          continue;
        }
        if let Some(module) = self.scanned_normal_module(idx) {
          stack.extend(
            self
              .resolved_records(module)
              .into_iter()
              .rev()
              .filter(|&(rec_idx, _)| module.import_records[rec_idx].kind == ImportKind::Import)
              .map(|(_, target)| target),
          );
        }
      }
    }
    let mut ordered = Vec::new();
    for pass_loading in [true, false] {
      for idx in &walk {
        if loading.contains(idx) == pass_loading
          && let Some(diagnostics) = failures.remove(idx)
        {
          ordered.extend(diagnostics);
        }
      }
    }
    let mut rest: Vec<_> = failures.into_iter().collect();
    rest.sort_unstable_by_key(|(idx, _)| (!loading.contains(idx), *idx));
    ordered.extend(rest.into_iter().flat_map(|(_, diagnostics)| diagnostics));
    ordered
  }

  fn scanned_link_failures(
    &self,
    loading_failures: &FxHashMap<ModuleIdx, Vec<BuildDiagnostic>>,
  ) -> FxHashMap<ModuleIdx, Vec<BuildDiagnostic>> {
    let mut failures: FxHashMap<ModuleIdx, Vec<BuildDiagnostic>> = FxHashMap::default();
    let candidate_indices: Option<Vec<ModuleIdx>> = if !self.is_full_scan
      && loading_failures.is_empty()
      && let HybridIndexVec::Map(ref scanned_modules) = self.intermediate_normal_modules.modules
    {
      if scanned_modules.is_empty() {
        return failures;
      }
      let can_check_only_scanned = self.new_added_modules_from_partial_scan.is_empty()
        && self.cache.pending_rescans.is_empty()
        && self.cache.snapshot().is_some_and(|snapshot| {
          scanned_modules.iter().all(|(&idx, m_opt)| {
            let Some(Module::Normal(new_n)) = m_opt.as_ref() else {
              return false;
            };
            let Some(Module::Normal(old_n)) = snapshot.module_table.modules.get(idx) else {
              return false;
            };
            old_n.exports_kind == rolldown_common::ExportsKind::Esm
              && new_n.exports_kind == rolldown_common::ExportsKind::Esm
              && !old_n.meta.has_lazy_export()
              && !new_n.meta.has_lazy_export()
              && !old_n.has_star_export()
              && !new_n.has_star_export()
              && old_n.named_exports.len() == new_n.named_exports.len()
              && old_n.named_exports.iter().all(|(name, old_exp)| {
                new_n.named_exports.get(name).is_some_and(|new_exp| {
                  !old_exp.came_from_commonjs
                    && !new_exp.came_from_commonjs
                    && match (
                      old_n.named_imports.get(&old_exp.referenced),
                      new_n.named_imports.get(&new_exp.referenced),
                    ) {
                      (None, None) => old_exp.referenced == new_exp.referenced,
                      (Some(old_imp), Some(new_imp)) => {
                        old_imp.imported == new_imp.imported
                          && (!matches!(new_imp.imported, rolldown_common::Specifier::Star)
                            || old_exp.span == new_exp.span)
                          && match (
                            old_n.import_records[old_imp.record_idx].resolved_module,
                            self.resolved_module(new_n, new_imp.record_idx),
                          ) {
                            (Some(old_target), Some(new_target)) => {
                              old_target == new_target
                                && !scanned_modules.contains_key(&new_target)
                                && !loading_failures.contains_key(&new_target)
                                && !self
                                  .cache
                                  .barrel_state
                                  .resolved_barrel_modules
                                  .contains_key(&new_target)
                            }
                            _ => false,
                          }
                      }
                      _ => false,
                    }
                })
              })
              && self
                .intermediate_normal_modules
                .importers
                .get(idx)
                .is_none_or(|importers| {
                  importers.iter().all(|imp| !self.cache.export_star_modules.contains(&imp.importer_idx))
                })
          })
        });
      can_check_only_scanned.then(|| {
        let mut idxs: Vec<ModuleIdx> = scanned_modules.keys().copied().collect();
        // A scan resolves records of cached barrels without rescanning them; their newly loaded
        // imports can expose a missing export too (#700).
        idxs.extend(self.cache.barrel_state.resolved_barrel_modules.keys().copied());
        idxs
      })
    } else {
      None
    };
    let all_indices = self.intermediate_normal_modules.importers.indices();
    let indices_iter: Box<dyn Iterator<Item = ModuleIdx>> = match candidate_indices {
      Some(vec) => Box::new(vec.into_iter()),
      None => Box::new(all_indices),
    };
    let mut resolution_cache: FxHashMap<(ModuleIdx, &str), ScannedExportResolution> =
      FxHashMap::default();
    for idx in indices_iter {
      let Some(module) = self.scanned_normal_module(idx) else {
        continue;
      };
      for import in module.named_imports.values() {
        let rolldown_common::Specifier::Literal(name) = &import.imported else {
          continue;
        };
        if module.import_records[import.record_idx].kind != ImportKind::Import {
          continue;
        }
        let Some(target) = self
          .resolved_module(module, import.record_idx)
          .and_then(|idx| self.scanned_normal_module(idx))
        else {
          continue;
        };
        let resolution = match resolution_cache.get(&(target.idx, name.as_str())) {
          Some(cached) => cached.clone(),
          None => {
            let resolved = self.resolve_scanned_export(
              target.idx,
              name,
              loading_failures,
              &mut FxHashSet::default(),
            );
            resolution_cache.insert((target.idx, name.as_str()), resolved.clone());
            resolved
          }
        };
        let diagnostic = match resolution {
          ScannedExportResolution::Unknown => continue,
          ScannedExportResolution::Bindings(bindings) if bindings.len() <= 1 => continue,
          ScannedExportResolution::Bindings(bindings) => {
            BuildDiagnostic::ambiguous_external_namespace(
              name.to_string(),
              target.stable_id.to_string(),
              AmbiguousExternalNamespaceModule {
                source: module.source.clone(),
                module_id: module.id.to_string(),
                stable_id: module.stable_id.to_string(),
                span_of_identifier: import.span_imported,
              },
              bindings
                .into_iter()
                .map(|(_, owner, span)| {
                  let owner = self.scanned_normal_module(owner).expect("resolved export exists");
                  AmbiguousExternalNamespaceModule {
                    source: owner.source.clone(),
                    module_id: owner.id.to_string(),
                    stable_id: owner.stable_id.to_string(),
                    span_of_identifier: span,
                  }
                })
                .collect(),
            )
          }
          ScannedExportResolution::Circular(owner, exported) => {
            BuildDiagnostic::circular_reexport(
              self.scanned_normal_module(owner).expect("circular export exists").id.to_string(),
              exported,
            )
          }
          ScannedExportResolution::Missing => {
            // Linking binds a missing export to `undefined` instead of failing the importer.
            if self.shared_context.options.shim_missing_exports {
              continue;
            }
            BuildDiagnostic::missing_export(
              module.id.to_string(),
              module.stable_id.to_string(),
              target.id.to_string(),
              target.stable_id.to_string(),
              module.source.clone(),
              name.to_string(),
              import.span_imported,
              None,
            )
          }
        };
        failures.entry(idx).or_default().push(diagnostic);
      }
    }
    failures
  }

  /// A parse failure belongs to module loading, not evaluation. Replace the
  /// dynamic root's entire invalid static closure before linking, otherwise an
  /// earlier valid sibling could run before the failed leaf throws.
  async fn isolate_dynamic_parse_errors(
    &mut self,
    entries: &FxHashSet<ModuleIdx>,
  ) -> BuildResult<FxHashSet<ModuleIdx>> {
    fn replace<T>(table: &mut HybridIndexVec<ModuleIdx, T>, idx: ModuleIdx, value: T) {
      match table {
        HybridIndexVec::IndexVec(values) => values[idx] = value,
        HybridIndexVec::Map(values) => {
          values.insert(idx, value);
        }
      }
    }
    let mut failures = std::mem::take(
      &mut *self.shared_context.deferred_parse_errors.lock().expect("parse failure state poisoned"),
    );
    for (id, diagnostic) in self
      .shared_context
      .deferred_resolution_targets
      .lock()
      .expect("resolution failure state poisoned")
      .iter()
    {
      if let Some(state) = self.cache.module_id_to_idx.get(id) {
        failures.entry(state.idx()).or_default().push(diagnostic.clone().into_diagnostic());
      }
    }
    let link_failures = self.scanned_link_failures(&failures);
    let link_messages = link_failures
      .iter()
      .map(|(&idx, diagnostics)| {
        // Messages become output bytes: stabilize paths against the build root,
        // never the compiling process's directory.
        (idx, diagnostics[0].to_message_with(&DiagnosticOptions { cwd: self.shared_context.options.cwd.clone() }))
      })
      .collect::<FxHashMap<_, _>>();
    let loading: FxHashSet<_> = failures.keys().copied().collect();
    for (idx, diagnostics) in link_failures {
      failures.entry(idx).or_default().extend(diagnostics);
    }
    if failures.is_empty() {
      return Ok(FxHashSet::default());
    }
    let mut affected: FxHashSet<_> = failures.keys().copied().collect();
    let mut queue: VecDeque<_> = affected.iter().copied().collect();
    let mut terminal = false;
    while let Some(idx) = queue.pop_front() {
      if entries.contains(&idx)
        || self
          .scanned_normal_module(idx)
          .is_some_and(|module| self.cache.user_defined_entry.contains(&module.id))
      {
        terminal = true;
      }
      for importer in &self.intermediate_normal_modules.importers[idx] {
        match importer.kind {
          ImportKind::DynamicImport => {}
          ImportKind::Import => {
            if affected.insert(importer.importer_idx) {
              queue.push_back(importer.importer_idx);
            }
          }
          // Conditional require and other host edges need their own loading
          // protocol; do not turn them into eagerly executed ESM dependencies.
          _ => terminal = true,
        }
      }
    }
    if terminal {
      // Loading every module may surface failures of modules that the graph no longer reaches
      // too, such as a missing export that a module imports which only a `require()` of a
      // dropped module loads, which a fresh build never loads (#690).
      self.cache.last_scan_failures_unreached = self.unreached(failures.keys().copied(), entries);
      return Err(self.in_first_parse_error_order(entries, &loading, failures).into());
    }

    // Retain the constructor in each eager entry's runtime dependency closure,
    // not just the lazy failure chunk. Capture it before authored modules can
    // replace the global constructor; finalization deconflicts lexical names.
    let runtime_idx = self.cache.module_id_to_idx[&RUNTIME_MODULE_ID].idx();
    let runtime_module = self.scanned_normal_module(runtime_idx).expect("runtime module exists");
    let constructors = [
      runtime_module.named_exports["__moduleSyntaxError"].referenced,
      runtime_module.named_exports["__moduleTypeError"].referenced,
      runtime_module.named_exports["__moduleLinkErrors"].referenced,
    ];
    let mut eager_entries = entries.clone();
    eager_entries.extend(
      self
        .cache
        .user_defined_entry
        .iter()
        .filter_map(|id| self.cache.module_id_to_idx.get(id).map(|state| state.idx())),
    );
    for idx in eager_entries {
      let scanned = match &self.intermediate_normal_modules.stmt_infos {
        HybridIndexVec::IndexVec(infos) => infos.get(idx).and_then(Option::as_ref),
        HybridIndexVec::Map(infos) => infos.get(&idx).and_then(Option::as_ref),
      };
      let mut infos = scanned
        .cloned()
        .or_else(|| self.cache.snapshot()?.stmt_infos.get(idx).cloned())
        .expect("entry statement information exists");
      for info in &mut infos.infos {
        info.referenced_symbols.extend(constructors.iter().copied().map(Into::into));
      }
      replace(&mut self.intermediate_normal_modules.stmt_infos, idx, Some(infos));
    }

    // Loading errors share the failed leaf. Tag compiler-generated linking
    // failures so each dynamic import can reject freshly without cloning authored
    // evaluation errors or cached parse errors.
    let link_error_source = |message: &str| {
      format!(
        "import {{__moduleLinkError}} from {:?}; throw __moduleLinkError({});",
        rolldown_common::RUNTIME_MODULE_KEY,
        serde_json::to_string(message).expect("diagnostic strings serialize"),
      )
    };
    let mut targets = Vec::new();
    for &root in &affected {
      if failures.contains_key(&root) {
        continue;
      }
      let mut stack = vec![root];
      let mut seen = FxHashSet::default();
      while let Some(idx) = stack.pop() {
        if !seen.insert(idx) {
          continue;
        }
        if failures.contains_key(&idx) {
          targets.push(match link_messages.get(&idx) {
            Some(message) => (root, runtime_idx, Some(link_error_source(message))),
            None => (root, idx, None),
          });
          break;
        }
        if let Some(module) = self.scanned_normal_module(idx) {
          stack.extend(
            self
              .resolved_records(module)
              .into_iter()
              .rev()
              .filter(|&(rec_idx, _)| module.import_records[rec_idx].kind == ImportKind::Import)
              .map(|(_, target)| target),
          );
        }
      }
    }
    for (idx, message) in link_messages {
      targets.push((idx, runtime_idx, Some(link_error_source(&message))));
    }
    for (idx, leaf, override_source) in targets {
      let mut module = self.scanned_normal_module(idx).expect("static ancestor exists").clone();
      let leaf_id = self.scanned_normal_module(leaf).expect("failed leaf exists").id.clone();
      let source = override_source.unwrap_or_else(|| {
        format!(
          "import {};",
          serde_json::to_string(leaf_id.as_str()).expect("module identifiers serialize")
        )
      });
      let mut warnings = Vec::new();
      let result = crate::ecmascript::ecma_module_view_factory::create_ecma_view(
        &mut crate::types::module_factory::CreateModuleContext {
          stable_id: &module.stable_id,
          module_idx: idx,
          plugin_driver: &self.shared_context.plugin_driver,
          resolved_id: &module.originative_resolved_id,
          options: &self.shared_context.options,
          warnings: &mut warnings,
          module_type: ModuleType::Js,
          replace_global_define_config: self
            .shared_context
            .meta
            .replace_global_define_config
            .clone(),
          is_user_defined_entry: false,
          flat_options: self.flat_options,
        },
        crate::types::module_factory::CreateModuleViewArgs {
          source: source.into(),
          sourcemap_chain: Vec::new(),
          hook_side_effects: Some(rolldown_common::side_effects::HookSideEffects::NoTreeshake),
        },
      )
      .await?;
      let old_dependencies: Vec<_> =
        self.resolved_records(&module).into_iter().map(|(_, dependency)| dependency).collect();
      for dep in old_dependencies {
        self.intermediate_normal_modules.importers[dep].retain(|record| record.importer_idx != idx);
        self.mark_module_importers_changed(dep);
      }
      self.intermediate_normal_modules.importers[leaf].push(ImporterRecord {
        kind: ImportKind::Import,
        importer_path: module.id.clone(),
        importer_idx: idx,
      });
      self.mark_module_importers_changed(leaf);
      if module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait) {
        self.tla_module_count = self.tla_module_count.saturating_sub(1);
      }
      self.tla_keyword_span_map.remove(&idx);
      module.ecma_view = result.ecma_view;
      module.ecma_view.imported_ids.insert(leaf_id);
      module.ecma_view.import_records = result
        .raw_import_records
        .into_iter()
        .map(|record| record.into_resolved(Some(leaf)))
        .collect();
      self.symbol_ref_db.store_local_db(idx, result.ecma_related.symbols);
      replace(
        &mut self.intermediate_normal_modules.index_ecma_ast,
        idx,
        Some(result.ecma_related.ast),
      );
      replace(
        &mut self.intermediate_normal_modules.stmt_infos,
        idx,
        Some(result.ecma_related.stmt_infos),
      );
      replace(&mut self.intermediate_normal_modules.modules, idx, Some(module.into()));
      self.cache.barrel_state.barrel_infos.remove(&idx);
      // Those of a barrel that the cache holds would land in the replacement's records.
      self.cache.barrel_state.resolved_barrel_modules.remove(&idx);
    }
    // The cached replacement graph no longer contains all authored edges.
    // Rescan every replaced node on the next revision, including unchanged
    // ancestors, so a repaired leaf restores the original graph and source.
    self.cache.pending_rescans = affected
      .iter()
      .filter_map(|&idx| {
        self
          .scanned_normal_module(idx)
          .filter(|module| {
            !module.id.as_str().starts_with(crate::deferred_resolution_error::FAILURE_MODULE_PREFIX)
          })
          .map(|module| module.originative_resolved_id.clone())
      })
      .collect();
    Ok(affected)
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  pub async fn resolve_user_defined_entries(
    &self,
  ) -> BuildResult<Vec<(Option<ArcStr>, ResolvedId)>> {
    let resolved_ids =
      join_all(self.shared_context.options.input.iter().map(|input_item| async move {
        let resolved = load_entry_module(
          &self.shared_context.resolver,
          &self.shared_context.plugin_driver,
          &input_item.import,
          None,
        )
        .await;

        resolved.map(|info| (input_item.name.as_ref().map(Into::into), info))
      }))
      .await;

    let mut ret = Vec::with_capacity(self.shared_context.options.input.len());

    let mut errors = vec![];

    for resolve_id in resolved_ids {
      match resolve_id {
        Ok(item) => {
          ret.push(item);
        }
        Err(e) => errors.push(e),
      }
    }

    if !errors.is_empty() {
      Err(errors)?;
    }

    Ok(ret)
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

  /// Loaded module `idx`, which this scan holds or the cache snapshot does.
  fn loaded(&self, idx: ModuleIdx) -> Option<&Module> {
    match &self.intermediate_normal_modules.modules {
      HybridIndexVec::IndexVec(modules) => modules.get(idx)?.as_ref(),
      HybridIndexVec::Map(modules) => match modules.get(&idx) {
        Some(module) => module.as_ref(),
        None => self.cache.get_snapshot().module_table.get(idx),
      },
    }
  }

  /// Loaded normal module `idx`; see [`Self::loaded`].
  fn loaded_module(&self, idx: ModuleIdx) -> Option<&rolldown_common::NormalModule> {
    self.loaded(idx)?.as_normal()
  }

  /// The modules that the `export *` records of loaded module `module` load so far.
  fn star_targets(&self, module: &rolldown_common::NormalModule) -> Vec<ModuleIdx> {
    if !module.has_star_export() {
      return Vec::new();
    }
    if !self.cache.barrel_state.resolved_barrel_modules.contains_key(&module.idx) {
      return module
        .import_records
        .iter()
        .filter(|rec| rec.meta.contains(ImportRecordMeta::IsExportStar))
        .filter_map(|rec| rec.resolved_module)
        .collect();
    }
    self
      .resolved_records(module)
      .into_iter()
      .filter(|&(rec_idx, _)| {
        module.import_records[rec_idx].meta.contains(ImportRecordMeta::IsExportStar)
      })
      .map(|(_, target)| target)
      .collect()
  }

  /// Whether module `idx`, as far as the scan loaded it, has exports that only the link can
  /// resolve: it is an external module or a CommonJS module, or an `export *` of it reaches one.
  fn dynamic_exports(&self, idx: ModuleIdx) -> bool {
    let mut pending = vec![idx];
    let mut visited = FxHashSet::default();
    while let Some(idx) = pending.pop() {
      if !visited.insert(idx) {
        continue;
      }
      match self.loaded(idx) {
        Some(Module::External(_)) => return true,
        Some(Module::Normal(module)) => {
          if module.exports_kind == ExportsKind::CommonJs {
            return true;
          }
          pending.extend(self.star_targets(module));
        }
        None => {}
      }
    }
    false
  }

  /// Whether a request asked loaded barrel `idx` for a name that it neither declares nor
  /// re-exports by name, which only its `export *` records can provide.
  fn asks_star_names(&self, idx: ModuleIdx) -> bool {
    let Some(ImportedExports::Partial(names)) = self.cache.barrel_state.requested_exports.get(&idx)
    else {
      return false;
    };
    self
      .loaded_module(idx)
      .is_some_and(|module| names.iter().any(|name| !module.named_exports.contains_key(name)))
  }

  /// Loads the plain imports of module `idx`, and of every barrel above it on a chain of
  /// `export *`, that a request asked for a name that only its stars provide (#661). `idx` turned
  /// out to have exports that only the link can resolve, which pass up the stars: an importer of
  /// such a name reads the barrel's namespace, whose use keeps the barrel's own statements
  /// (tree shaking's body demand), which read those imports. A name that a star gets from an ES
  /// module binds to its export instead, which leaves them unloaded.
  fn load_bodies_above(
    &self,
    idx: ModuleIdx,
    work_queue: &mut VecDeque<(ModuleIdx, ImportedExports, Demand)>,
  ) {
    let mut pending = vec![idx];
    let mut visited = FxHashSet::default();
    while let Some(idx) = pending.pop() {
      if !visited.insert(idx) {
        continue;
      }
      if matches!(self.cache.barrel_state.barrel_infos.get(&idx), Some(Some(_)))
        && self.asks_star_names(idx)
      {
        work_queue.push_back((idx, ImportedExports::Partial(FxHashSet::default()), Demand::Body));
      }
      let Some(importers) = self.intermediate_normal_modules.importers.get(idx) else {
        continue;
      };
      pending.extend(
        importers
          .iter()
          .filter(|importer| {
            importer.kind == ImportKind::Import
              && self
                .loaded_module(importer.importer_idx)
                .is_some_and(|module| self.star_targets(module).contains(&idx))
          })
          .map(|importer| importer.importer_idx),
      );
    }
  }

  /// The import records of loaded module `idx` that only re-export.
  fn reexport_records(&self, idx: ModuleIdx) -> Option<FxHashSet<ImportRecordIdx>> {
    let module = self.loaded_module(idx)?;
    Some(
      module
        .import_records
        .iter_enumerated()
        .filter(|(_, rec)| is_reexport(rec.meta))
        .map(|(rec_idx, _)| rec_idx)
        .collect(),
    )
  }

  #[expect(clippy::rc_buffer)]
  fn process_barrel_import_record(
    &mut self,
    work_queue: &mut VecDeque<(ModuleIdx, ImportedExports, Demand)>,
    user_defined_entries: &Arc<Vec<(Option<ArcStr>, ResolvedId)>>,
  ) {
    while let Some((idx, imported_exports, demand)) = work_queue.pop_front() {
      let runs = demand == Demand::Run;
      // A module that an `import()` runs takes every export if it has an `export *` or exports a
      // `then` that the import may call (`take_body_records`).
      let imported_exports = if runs
        && (matches!(
          self.cache.barrel_state.barrel_infos.get(&idx),
          Some(Some(state)) if !state.info.star.is_empty()
        ) || self.loaded_module(idx).is_some_and(exports_callable_then))
      {
        ImportedExports::All
      } else {
        imported_exports
      };
      // Calculate new exports to process (subtract already processed). A request of the module's
      // own statements names none.
      let new_exports = if demand == Demand::Body {
        ImportedExports::Partial(FxHashSet::default())
      } else {
        match self.cache.barrel_state.requested_exports.entry(idx) {
          Entry::Occupied(mut occ) => {
            match imported_exports.subtract_and_merge_into(occ.get_mut()) {
              Some(diff) => diff,
              None if runs => ImportedExports::Partial(FxHashSet::default()),
              None => continue,
            }
          }
          Entry::Vacant(vac) => {
            vac.insert(imported_exports.clone());
            imported_exports
          }
        }
      };
      // A module that an `import()` runs, or whose dynamic exports keep its own statements
      // (`load_bodies_above`), also needs its plain imports (`take_body_records`). If its records
      // can't be read, every remaining record counts as one.
      let reexports =
        (demand != Demand::Exports).then(|| self.reexport_records(idx).unwrap_or_default());

      // No entry in `barrel_infos` means this module hasn't been loaded and processed yet.
      // Record it for now and process later when loading is complete.
      let Some(Some(barrel_module_state)) = self.cache.barrel_state.barrel_infos.get_mut(&idx)
      else {
        continue;
      };

      // No tracked records, skip
      if barrel_module_state.tracked_records.is_empty() {
        continue;
      }

      let mut tracked_records = std::mem::take(&mut barrel_module_state.tracked_records);
      let mut remaining_imported_specifiers =
        std::mem::take(&mut barrel_module_state.remaining_imported_specifiers);
      let stars = barrel_module_state.info.star.clone();
      // The modules that the module's `export *` records load now.
      let mut star_targets = Vec::new();

      // Below repeats the post-module-load processing logic, but only for the remaining `tracked_records`.
      // Additionally, we check whether each record in `tracked_records` still needs to be retained.
      let mut needed_records = barrel_module_state
        .info
        .take_needed_records(&new_exports, &mut remaining_imported_specifiers);
      if let Some(reexports) = &reexports {
        take_body_records(
          |rec_idx| reexports.contains(&rec_idx),
          &mut remaining_imported_specifiers,
          &mut needed_records,
        );
      }

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
              user_defined_entries,
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
        if stars.contains(&rec_idx) {
          star_targets.push(target_idx);
        }
        let keep_tracking = match self.cache.barrel_state.barrel_infos.get(&target_idx) {
          Some(Some(s)) => target_idx == idx || !s.tracked_records.is_empty(),
          Some(None) => false,
          None => true,
        };
        work_queue.push_back((target_idx, needed_record, Demand::Exports));
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
      // A star that reaches a module that the scan loaded with dynamic exports gives this module
      // dynamic exports too.
      if star_targets.iter().any(|&target| self.dynamic_exports(target)) {
        self.load_bodies_above(idx, work_queue);
      }
    }
  }
}

/// What a request of a module needs besides the exports that it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Demand {
  /// Only those exports.
  Exports,
  /// An `import()` runs the module as an entry of its own (`take_body_records`).
  Run,
  /// The module's own statements, which its dynamic exports keep (`load_bodies_above`).
  Body,
}

/// Whether an import record only re-exports, which binds nothing that the module's own statements
/// read.
fn is_reexport(meta: ImportRecordMeta) -> bool {
  meta.intersects(ImportRecordMeta::IsReExportOnly | ImportRecordMeta::IsExportStar)
}

/// Moves a barrel's plain import records from `remaining` to `needed`, when an `import()` runs the
/// barrel as an entry of its own: its statements may read what those records bind, whatever names
/// the import reads, as when an importer uses one of its own exports
/// (`BarrelInfo::take_needed_records`). The records that only re-export load by name (#502). A
/// barrel with an `export *` loads every record instead: a CommonJS module behind the star gives
/// it dynamic exports, which keep its whole namespace, its statements and the `then` that resolving
/// the import reads, also in a barrel that the star reaches. So does a barrel that exports a `then`
/// that the import may call, whose whole namespace the import keeps (#664).
/// Whether `module` exports a `then` that resolving an `import()` of it may call, which keeps its
/// whole namespace (`LinkStage::keep_thenable_namespaces`).
fn exports_callable_then(module: &rolldown_common::NormalModule) -> bool {
  module.named_exports.contains_key("then") && !exports_namespace_as_then(module)
}

fn take_body_records(
  is_reexport: impl Fn(ImportRecordIdx) -> bool,
  remaining: &mut FxHashMap<ImportRecordIdx, ImportedExports>,
  needed: &mut FxHashMap<ImportRecordIdx, ImportedExports>,
) {
  remaining.retain(|&rec_idx, exports| {
    if is_reexport(rec_idx) {
      return true;
    }
    needed
      .entry(rec_idx)
      .and_modify(|needed| needed.merge(exports))
      .or_insert_with(|| exports.clone());
    false
  });
}
