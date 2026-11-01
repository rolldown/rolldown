//! A fast native relink re-renders only the native records that an update changed instead of
//! linking and generating the whole graph again. It inserts modules that partial scans appended
//! into the cached link output and retires the modules that the update leaves unreachable, so
//! the cached tables keep stable indices. Whatever it cannot prove equal to a fresh link falls
//! back to linking the whole graph.

use std::path::Path;

use oxc::ast::ast::{Declaration, ExportDefaultDeclarationKind, ImportPhase, Statement};
use oxc_index::IndexVec;
use rolldown_common::{
  ChunkIdx, DeclaredSymbols, DependedRuntimeHelperMap, EcmaModuleAstUsage, EcmaViewMeta,
  EntryPointKind, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta,
  MemberExprRefResolutionMap, Module, ModuleIdx, ModuleNamespaceIncludedReason, ModuleType,
  NamespaceAlias, RUNTIME_HELPER_NAMES, ResolvedExport, RuntimeHelper, Specifier, StmtInfo,
  StmtInfoIdx, StmtInfoMeta, StmtInfos, SymbolOrMemberExprRef, SymbolRef, SymbolRefDbForModule,
  SymbolRefFlags, TaggedSymbolRef, WrapKind, side_effects::DeterminedSideEffects,
};
use rolldown_error::EventKindSwitcher;
use rolldown_fs::FileSystem;
use rolldown_utils::{
  IndexBitSet, concat_string,
  indexmap::{FxIndexMap, FxIndexSet},
};
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::smallvec;

use super::bundle::Bundle;
use crate::{
  stages::{
    generate_stage::{common_dir_input, is_live_chunk},
    link_stage::{Reachability, UnreachableModules, linked_side_effects},
    scan_stage::NormalizedScanStageOutput,
  },
  types::{linking_metadata::LinkingMetadata, scan_stage_cache::CachedNativeRelinkState},
  utils::external_import_interop::import_record_needs_interop,
};

thread_local! {
  /// The label of the last check that rejected a fast native relink that this thread prepared.
  /// Preparing runs on the thread that asks for it, with no parallelism inside.
  static REJECTION: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
  /// What that check rejected, for the checks that name the modules involved.
  static REJECTION_DETAIL: std::cell::RefCell<Option<String>> =
    const { std::cell::RefCell::new(None) };
}

/// Rejects a fast native relink, saying why with `$label`, and what with `$detail`.
macro_rules! reject_with {
  ($label:expr) => {{
    tracing::trace!(target: "native_relink", reason = $label, "fast native relink rejected");
    REJECTION.set(Some($label));
    REJECTION_DETAIL.set(None);
    return None;
  }};
  ($label:expr, $detail:expr) => {{
    tracing::trace!(target: "native_relink", reason = $label, "fast native relink rejected");
    REJECTION.set(Some($label));
    REJECTION_DETAIL.set(Some($detail));
    return None;
  }};
}

macro_rules! check_or_none {
  ($cond:expr, $label:expr) => {
    if !($cond) {
      reject_with!($label);
    }
  };
  ($cond:expr, $label:expr, $detail:expr) => {
    if !($cond) {
      reject_with!($label, $detail);
    }
  };
}

/// Takes the label of the last check that rejected a fast native relink on this thread.
pub(super) fn take_rejection() -> Option<&'static str> {
  REJECTION.take()
}

/// Takes what the last rejection on this thread rejected, when its check named it.
pub(super) fn take_rejection_detail() -> Option<String> {
  REJECTION_DETAIL.take()
}

/// The link data of one changed or appended module, computed against the cached link output.
pub(super) struct PreparedDirtyModule {
  pub idx: ModuleIdx,
  pub chunk_idx: ChunkIdx,
  pub exec_order: u32,
  pub post_link_exports_kind: ExportsKind,
  pub post_link_side_effects: DeterminedSideEffects,
  pub new_stmt_infos: StmtInfos,
  pub new_local_db: SymbolRefDbForModule,
  pub cjs_namespace_renames: Vec<(oxc::semantic::SymbolId, String)>,
  pub stmt_info_included: IndexBitSet<StmtInfoIdx>,
  pub local_used_symbols: FxHashSet<SymbolRef>,
  pub new_resolved_member_expr_refs: MemberExprRefResolutionMap,
  pub new_folded_namespace_in_checks: FxHashMap<oxc::syntax::node::NodeId, bool>,
  /// A partial scan appended the module since the cached link.
  pub added: bool,
  pub depended_runtime_helper: RuntimeHelper,
  pub native_default_expression: bool,
  /// Old to new refs of the module's symbols that its scan renumbered and other cached link
  /// data names: exports, its namespace and default bindings, named imports and import records.
  pub renamed: FxHashMap<SymbolRef, SymbolRef>,
  /// Its import records gained edges to appended modules or lost edges to retired ones.
  pub edges_changed: bool,
  /// The modules in the output, neither appended nor retired, that its new records import or
  /// its dropped records imported (#626).
  pub edge_targets: Vec<ModuleIdx>,
  /// Its CommonJS interop maps, keyed by its new symbols, when its new records import appended
  /// CommonJS modules; they replace the cached ones once its renamed symbols are (#653).
  pub cjs_interop: Option<(FxHashMap<SymbolRef, ModuleIdx>, FxHashMap<SymbolRef, ModuleIdx>)>,
  /// An appended CommonJS module's `require_*` wrapper, which its scope tree gains (#653).
  pub wrapper: Option<(oxc::semantic::SymbolId, String)>,
  /// What it includes of appended CommonJS modules, which they use (#653).
  pub commonjs_uses: Vec<SymbolRef>,
}

/// How an update changes the module graph of the cached link output.
pub(super) struct NativeGraphPlan {
  /// Appended modules and retired modules that the update rescanned and reaches again, in index
  /// order.
  pub added: Vec<ModuleIdx>,
  /// Modules that the update leaves unreachable; they stay in the tables, without a chunk.
  pub retired: Vec<ModuleIdx>,
  /// Every module's execution order in the updated graph, `u32::MAX` when unreachable.
  pub exec_orders: IndexVec<ModuleIdx, u32>,
  /// The modules that the updated graph's entries don't reach, which linking passes over.
  pub unreachable: UnreachableModules,
  pub sorted_modules: Vec<ModuleIdx>,
  /// Whether cached modules execute in another relative order than the cached output's.
  pub reordered: bool,
  /// The chunk of each module in `added`, appended to the chunk table in `added` order.
  pub added_chunks: FxHashMap<ModuleIdx, ChunkIdx>,
  /// Cached modules whose importer sets changed, to copy from the scan.
  pub importer_changes: Vec<ModuleIdx>,
}

pub(super) struct PreparedFastNativeRelink {
  pub modules: Vec<PreparedDirtyModule>,
  pub graph: Option<NativeGraphPlan>,
  /// Changed modules that the updated graph does not reach. They render no record; their scans
  /// replace the cached link data, which linking the whole graph would merge back.
  pub unreachable: Vec<ModuleIdx>,
  /// Cached CommonJS modules, with their chunks, whose records the relink renders again with
  /// their cached link data, since the update makes them start or stop calling their wrappers as
  /// they evaluate (`keeps_imported_link_data`, #684).
  pub rerendered: Vec<(ModuleIdx, ChunkIdx)>,
}

struct PrepareContext {
  added: FxHashSet<ModuleIdx>,
  retired: FxHashSet<ModuleIdx>,
  exec_orders: Option<IndexVec<ModuleIdx, u32>>,
  /// The modules that the updated graph's entries don't reach, when the update changes edges.
  unreachable: Option<UnreachableModules>,
  added_chunks: FxHashMap<ModuleIdx, ChunkIdx>,
  /// Development output without tree shaking, where edges may change and scans may renumber
  /// symbols.
  structural: bool,
  /// Symbols used by the appended modules prepared so far.
  fresh_used: FxHashMap<ModuleIdx, FxHashSet<SymbolRef>>,
  /// Changed modules whose linked side effects the update changes (`changed_linked_side_effects`).
  side_effects_changed: FxHashSet<ModuleIdx>,
}

impl PrepareContext {
  /// Whether linking the updated graph marks `symbol` used. Modules relinked since the cached
  /// link answer with the symbols their included statements reach.
  fn is_used(&self, cached: &CachedNativeRelinkState, symbol: SymbolRef) -> bool {
    if let Some(used) = self.fresh_used.get(&symbol.owner) {
      return used.contains(&symbol);
    }
    if let Some(used) = cached.generate_state.used_overrides.get(&symbol.owner) {
      return used.contains(&symbol);
    }
    cached.generate_state.used_symbol_refs.contains(&symbol)
  }
}

/// Whether a module's static and dynamic edges differ between two scans.
fn import_edges_changed(
  old: &rolldown_common::NormalModule,
  new: &rolldown_common::NormalModule,
) -> bool {
  old.import_records.len() != new.import_records.len()
    || old
      .import_records
      .iter()
      .zip(new.import_records.iter())
      .any(|(old, new)| old.kind != new.kind || old.resolved_module != new.resolved_module)
}

/// The part of a module path below the common output directory `base`, when the path lies
/// strictly inside it.
fn below_input_base<'a>(path: &'a str, base: &str) -> Option<std::path::Components<'a>> {
  if base.is_empty() {
    return None;
  }
  let rest = Path::new(path).strip_prefix(base).ok()?;
  rest.components().next().is_some().then(|| rest.components())
}

/// Whether a cached module is a retired one that left the output entirely: no execution order,
/// no inclusion and no chunk. Rescanned, it can return as an appended module.
fn is_refreshed_candidate(cached: &CachedNativeRelinkState, idx: ModuleIdx) -> bool {
  cached.link_output.module_table.modules[idx].exec_order() == u32::MAX
    && !cached.link_output.metas[idx].is_included
    && cached.generate_state.chunk_graph.module_to_chunk.get(idx).copied().flatten().is_none()
}

/// Whether two side-effect classifications agree.
fn same_side_effects(a: DeterminedSideEffects, b: DeterminedSideEffects) -> bool {
  std::mem::discriminant(&a) == std::mem::discriminant(&b)
    && a.has_side_effects() == b.has_side_effects()
}

/// Whether linking the updated graph gives every module that it reaches the side effects that
/// the cached link gave it; `exec_orders` orders the updated graph, when the update changed its
/// edges. Checking a changed module against its dependencies' cached side effects does not
/// suffice: the modules of a cycle that had side effects only through a dependency would vouch
/// for each other once an update dropped that dependency. Linking derives side effects from the
/// graph alone, so the modules that the update retires cannot keep any.
///
/// Returns the changed modules whose side effects do change, as an activated lazy proxy's, which
/// imports its target from then on (#651), or `None` if another module's do. Only a changed
/// module's own record, which the relink renders again, may read them: no module may import it
/// statically, before or after the update (`unused_side_effect_free_import`), nor through a
/// top-level `import()` whose namespace goes unused, which keeps its target only if the target
/// has side effects (`is_dynamic_entry_alive`).
fn changed_linked_side_effects(
  cached: &CachedNativeRelinkState,
  snapshot: &NormalizedScanStageOutput,
  exec_orders: Option<&IndexVec<ModuleIdx, u32>>,
  changed: &[ModuleIdx],
) -> Option<FxHashSet<ModuleIdx>> {
  let modules = &cached.link_output.module_table.modules;
  let scanned_modules = &snapshot.module_table.modules;
  let pure_import = |table: &IndexVec<ModuleIdx, Module>, importer: ModuleIdx, idx: ModuleIdx| {
    table.get(importer).and_then(Module::as_normal).is_some_and(|importer| {
      importer.import_records.iter().any(|record| {
        record.resolved_module == Some(idx)
          && record.meta.contains(ImportRecordMeta::TopLevelPureDynamicImport)
      })
    })
  };
  let mut flipped = FxHashSet::default();
  for (idx, &has_side_effects) in
    linked_side_effects(scanned_modules, &cached.link_output.metas).iter_enumerated()
  {
    let Some(module) = modules.get(idx).and_then(Module::as_normal) else {
      continue;
    };
    if exec_orders.map_or(module.exec_order, |orders| orders[idx]) == u32::MAX
      || module.side_effects.has_side_effects() == has_side_effects
    {
      continue;
    }
    let scanned = scanned_modules[idx].as_normal()?;
    if !changed.contains(&idx)
      || !module.ecma_view.importers_idx.is_empty()
      || !scanned.ecma_view.importers_idx.is_empty()
      || module
        .ecma_view
        .dynamic_importers_idx
        .iter()
        .chain(&scanned.ecma_view.dynamic_importers_idx)
        .any(|&importer| {
          pure_import(modules, importer, idx) || pure_import(scanned_modules, importer, idx)
        })
    {
      return None;
    }
    flipped.insert(idx);
  }
  Some(flipped)
}

impl<Fs: FileSystem + Clone + 'static> Bundle<Fs> {
  /// Computes the link data of every changed and appended module against the cached link
  /// output, which this seeds with the appended modules. `None` when the update is not
  /// provably equal to linking the whole graph; the caller then discards the cached output.
  #[inline(never)]
  pub(super) fn try_prepare_fast_native_relink(
    &self,
    cached: &mut CachedNativeRelinkState,
  ) -> Option<PreparedFastNativeRelink> {
    let snapshot = self.cache.snapshot()?;
    let old_len = cached.link_output.module_table.modules.len();
    let added = self.cache.added_native_modules.clone();
    check_or_none!(snapshot.module_table.modules.len() == old_len + added.len(), "modules_len");
    check_or_none!(
      added.iter().enumerate().all(|(position, idx)| idx.index() == old_len + position),
      "appended_modules"
    );
    check_or_none!(snapshot.tla_module_count == 0, "tla_count");
    check_or_none!(snapshot.tla_keyword_span_map.is_empty(), "tla_span");
    check_or_none!(
      snapshot.user_defined_entry_modules == cached.link_output.user_defined_entry_modules,
      "user_defined_entries"
    );

    let mut dirty_modules: Vec<ModuleIdx> =
      self.cache.dirty_native_modules.iter().copied().filter(|idx| idx.index() < old_len).collect();
    dirty_modules.sort_unstable();
    // A module that the graph reaches only through records that order no execution, an
    // `import.meta.hot.accept()` dependency and what only such modules import, has no record, so
    // the relink prepares nothing of it, while linking reads its records as any module's (#660).
    // Applications rarely name such dependencies, so an update that edits one, or that appends,
    // retires or revives one, or gives it a record or takes it away, links the whole graph.
    if let Some(&idx) = dirty_modules.iter().find(|&&idx| {
      cached.link_output.module_table.modules[idx].exec_order() == u32::MAX
        && cached.link_output.unreachable_modules.reaches(idx)
    }) {
      reject_with!(
        "recordless_module",
        cached.link_output.module_table.modules[idx].stable_id().to_string()
      );
    }
    // An update that can change how an import resolves into a module whose records merged their
    // CommonJS interop namespaces links the whole graph (#683).
    let merged_reach = MergedNamespaceReach::of(cached);
    if !merged_reach.bindings.is_empty() {
      check_merged_namespace_edits(cached, snapshot, &dirty_modules, &merged_reach)?;
    }

    let structural = self.options.is_dev_mode_enabled()
      && self.options.treeshake.is_none()
      && !cached.link_output.symbol_db.has_module_preserve_jsx();
    let edges_changed = !added.is_empty()
      || !self.cache.native_importer_changes.is_empty()
      || dirty_modules.iter().any(|&idx| {
        match (
          cached.link_output.module_table.modules[idx].as_normal(),
          snapshot.module_table.modules[idx].as_normal(),
        ) {
          (Some(old), Some(new)) => import_edges_changed(old, new),
          _ => true,
        }
      });
    // A changed module whose own side effects changed can change those that linking derives
    // for the modules that import it.
    let side_effects_rescanned = dirty_modules.iter().any(|&idx| {
      cached.scanned_side_effects.get(idx).is_none_or(|scanned| {
        !same_side_effects(*scanned, *snapshot.module_table.modules[idx].side_effects())
      })
    });

    let mut ctx = PrepareContext {
      added: FxHashSet::default(),
      retired: FxHashSet::default(),
      exec_orders: None,
      unreachable: None,
      added_chunks: FxHashMap::default(),
      structural,
      fresh_used: FxHashMap::default(),
      side_effects_changed: FxHashSet::default(),
    };
    let mut graph = None;
    let mut rerendered = Vec::new();
    if edges_changed {
      check_or_none!(structural, "structural");
      let mut plan = self.plan_native_graph(cached, snapshot, &added)?;
      if !merged_reach.bindings.is_empty() {
        check_merged_namespace_edges(cached, snapshot, &plan, &merged_reach)?;
      }
      check_or_none!(self.keeps_input_base(cached, snapshot, &plan), "input_base");
      plan.importer_changes = self.check_native_importer_changes(cached, snapshot, &plan)?;
      self.check_retired_modules(cached, snapshot, &plan)?;
      for idx in self.keeps_imported_link_data(cached, snapshot, &plan)? {
        // Chunk decisions assume that their dependencies evaluate as they did, strict execution
        // order initializes entries through its own lowering, and linking lowers a lazy export,
        // as a JSON or text module's, into the module's AST (`generate_lazy_export`), which the
        // cache holds as the scan left it.
        let chunk_idx = cached.generate_state.chunk_graph.module_to_chunk.get(idx).copied().flatten();
        let Some(chunk_idx) = chunk_idx.filter(|_| {
          !cached.generate_state.decision_dependencies.contains(&idx)
            && !self.options.is_strict_execution_order_enabled()
            && !cached.link_output.module_table.modules[idx]
              .as_normal()
              .is_some_and(|module| module.meta.has_lazy_export())
        }) else {
          reject_with!(
            "commonjs_evaluation",
            cached.link_output.module_table.modules[idx].stable_id().to_string()
          );
        };
        rerendered.push((idx, chunk_idx));
      }
      self.seed_added_modules(cached, snapshot, &plan)?;
      self.release_retired_interop(cached, snapshot, &plan);
      let Some(flipped) =
        changed_linked_side_effects(cached, snapshot, Some(&plan.exec_orders), &dirty_modules)
      else {
        reject_with!("linked_side_effects");
      };
      ctx.side_effects_changed = flipped;
      check_or_none!(self.keeps_inclusion(cached, snapshot, &plan), "inclusion");
      ctx.added = plan.added.iter().copied().collect();
      dirty_modules.retain(|idx| !ctx.added.contains(idx));
      ctx.retired = plan.retired.iter().copied().collect();
      ctx.exec_orders = Some(plan.exec_orders.clone());
      ctx.unreachable = Some(plan.unreachable.clone());
      ctx.added_chunks.clone_from(&plan.added_chunks);
      graph = Some(plan);
    } else if side_effects_rescanned {
      let Some(flipped) = changed_linked_side_effects(cached, snapshot, None, &dirty_modules)
      else {
        reject_with!("linked_side_effects");
      };
      ctx.side_effects_changed = flipped;
    }

    if self.options.is_dev_mode_enabled() && crate::utils::native_records::enabled(&self.options) {
      let mut row_dirty = FxHashSet::default();
      for &idx in &dirty_modules {
        let (
          Some(old),
          Some(new),
        ) = (
          cached.link_output.module_table.modules[idx].as_normal(),
          snapshot.module_table.modules[idx].as_normal(),
        )
        else {
          row_dirty.insert(idx);
          continue;
        };
        if old.is_hmr_partially_accepting_module() != new.is_hmr_partially_accepting_module() {
          row_dirty.insert(idx);
          row_dirty.extend(old.ecma_view.importers_idx.iter().copied());
        }
        let has_partially_accepting_target = new.import_records.iter().any(|rec| {
          rec.kind.is_static()
            && rec.resolved_module.is_some_and(|target| {
              snapshot
                .module_table
                .modules
                .get(target)
                .and_then(Module::as_normal)
                .is_some_and(rolldown_common::NormalModule::is_hmr_partially_accepting_module)
            })
        });
        if import_edges_changed(old, new)
          || (has_partially_accepting_target
            && crate::hmr::module_graph_delta::static_import_bindings(old)
              != crate::hmr::module_graph_delta::static_import_bindings(new))
        {
          row_dirty.insert(idx);
        }
      }
      if !row_dirty.is_empty() {
        for (chunk_idx, chunk) in
          cached.generate_state.chunk_graph.chunk_table.iter_enumerated()
        {
          if !is_live_chunk(&cached.generate_state.chunk_graph, chunk_idx) {
            continue;
          }
          let rows = crate::utils::native_records::dev_graph_rows(
            chunk_idx,
            chunk,
            &cached.link_output.module_table,
            &cached.generate_state.chunk_graph,
            &cached.link_output.metas,
            true,
          );
          if !rows.iter().skip(1).any(|row| row_dirty.contains(row)) {
            continue;
          }
          let [idx] = chunk.modules.as_slice() else {
            reject_with!("dev_graph_multi_module");
          };
          let idx = *idx;
          if ctx.retired.contains(&idx)
            || ctx.added.contains(&idx)
            || self.cache.dirty_native_modules.contains(&idx)
            || rerendered.iter().any(|(existing, _)| *existing == idx)
          {
            continue;
          }
          check_or_none!(
            !cached.generate_state.decision_dependencies.contains(&idx)
              && !self.options.is_strict_execution_order_enabled()
              && !cached.link_output.module_table.modules[idx]
                .as_normal()
                .is_some_and(|module| module.meta.has_lazy_export()),
            "dev_graph_rerender",
            cached.link_output.module_table.modules[idx].stable_id().to_string()
          );
          rerendered.push((idx, chunk_idx));
        }
      }
    }

    let mut prepared = Vec::with_capacity(dirty_modules.len() + ctx.added.len());
    // Appended modules first, dependencies before their importers, which read their link data.
    let mut added_in_order: Vec<ModuleIdx> = ctx.added.iter().copied().collect();
    added_in_order.sort_unstable();
    if let Some(exec_orders) = &ctx.exec_orders {
      added_in_order.sort_unstable_by_key(|idx| exec_orders[*idx]);
    }
    for idx in added_in_order {
      let item = self.prepare_native_module(cached, snapshot, idx, &ctx)?;
      cached.link_output.stmt_infos[idx] = item.new_stmt_infos.clone();
      let meta = &mut cached.link_output.metas[idx];
      meta.stmt_info_included = item.stmt_info_included.clone();
      meta.resolved_member_expr_refs = item.new_resolved_member_expr_refs.clone();
      meta.depended_runtime_helper = item.depended_runtime_helper;
      meta.native_default_expression = item.native_default_expression;
      // A refreshed module's stale retained exports must agree with the ones it now uses.
      check_or_none!(
        idx.index() >= old_len
          || cached.link_output.metas[idx].resolved_exports.values().all(|export| {
            !cached.link_output.retained_export_symbols.contains(&export.symbol_ref)
              || item.local_used_symbols.contains(&export.symbol_ref)
          }),
        "revived_retained"
      );
      ctx.fresh_used.insert(idx, item.local_used_symbols.clone());
      prepared.push(item);
    }
    let mut unreachable = Vec::new();
    for idx in dirty_modules {
      let reachable = ctx.exec_orders.as_ref().map_or_else(
        || cached.link_output.module_table.modules[idx].exec_order() != u32::MAX,
        |exec_orders| exec_orders[idx] != u32::MAX,
      );
      if reachable {
        prepared.push(self.prepare_native_module(cached, snapshot, idx, &ctx)?);
      } else {
        unreachable.push(idx);
      }
    }
    // Linking marks used what the relinked modules include of the appended CommonJS modules,
    // and includes each one's wrapper only if a record that imports or requires it calls it.
    // What an appended one re-exports of a module in the output must already be used.
    let commonjs_uses = prepared
      .iter()
      .flat_map(|item| item.commonjs_uses.iter().map(|symbol| (item.idx, *symbol)))
      .collect::<Vec<_>>();
    check_or_none!(
      commonjs_uses
        .iter()
        .all(|(_, used)| ctx.added.contains(&used.owner) || ctx.is_used(cached, *used)),
      "added_cjs_bailout"
    );
    for item in &mut prepared {
      if let Some((wrapper, _)) = item.wrapper {
        check_or_none!(
          commonjs_uses.iter().any(|(user, used)| *user != item.idx
            && used.symbol == wrapper
            && used.owner == item.idx),
          "added_commonjs_unreferenced",
          snapshot.module_table.modules[item.idx].stable_id().to_string()
        );
        item.local_used_symbols.extend(
          commonjs_uses.iter().filter(|(_, used)| used.owner == item.idx).map(|(_, used)| *used),
        );
      }
    }
    Some(PreparedFastNativeRelink { modules: prepared, graph, unreachable, rerendered })
  }

  /// Orders the updated graph's modules as `LinkStage::sort_modules` would, and finds the
  /// modules that the update appends to and retires from the output. An update may reorder the
  /// execution of cached modules (#597). Their records depend on that order only through the
  /// order of their imports, which the chunk graph's update checks, unless the graph has an
  /// external module, whose imports and default export name follow it, an ESM-wrapped module,
  /// whose initializers importers call in that order, or order wrapping.
  fn plan_native_graph(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    added: &[ModuleIdx],
  ) -> Option<NativeGraphPlan> {
    enum Step {
      Enter(ModuleIdx),
      Exit(ModuleIdx),
    }
    let modules = &snapshot.module_table.modules;
    let old_modules = &cached.link_output.module_table.modules;
    let old_len = old_modules.len();

    // Linking passes over the modules that the entries no longer reach, and the dynamic entries
    // that they alone import (`LinkStage::exclude_unreachable_modules`).
    let reachability = Reachability::new(
      modules,
      snapshot
        .entry_points
        .iter()
        .filter(|entry| entry.kind.is_user_defined())
        .map(|entry| entry.idx)
        .chain(snapshot.user_defined_entry_modules.iter().copied())
        .chain(std::iter::once(cached.link_output.runtime.id())),
    );
    // `LinkStage::new` keeps user-defined entries first and sorts the others by kind and id.
    let mut roots = Vec::new();
    let mut rest = Vec::new();
    for entry in snapshot.entry_points.iter().filter(|entry| reachability.keeps_entry(entry)) {
      if matches!(entry.kind, EntryPointKind::UserDefined) {
        roots.push(entry.idx);
      } else {
        rest.push((entry.kind, modules[entry.idx].id().as_str(), entry.idx));
      }
    }
    rest.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    roots.extend(rest.into_iter().map(|(_, _, idx)| idx));
    let mut seen_roots = FxHashSet::default();
    roots.retain(|idx| seen_roots.insert(*idx));

    let code_splitting_disabled = self.options.code_splitting.is_disabled();
    let report_cycles = self.options.checks.contains(EventKindSwitcher::CircularDependency);
    let mut stack: Vec<Step> = roots
      .iter()
      .rev()
      .map(|&idx| Step::Enter(idx))
      .chain(std::iter::once(Step::Enter(cached.link_output.runtime.id())))
      .collect();
    let mut state = vec![0u8; modules.len()];
    let mut exec_orders = IndexVec::from_vec(vec![u32::MAX; modules.len()]);
    let mut order = Vec::with_capacity(modules.len());
    let mut next_exec_order = 0u32;
    while let Some(step) = stack.pop() {
      match step {
        Step::Enter(idx) => match state[idx.index()] {
          0 => {
            state[idx.index()] = 1;
            stack.push(Step::Exit(idx));
            stack.extend(
              modules[idx]
                .import_records()
                .iter()
                .filter(|rec| {
                  rec.kind.is_static() || (code_splitting_disabled && rec.kind.is_dynamic())
                })
                .filter_map(|rec| rec.resolved_module)
                .rev()
                .map(Step::Enter),
            );
          }
          // Linking would report the cycle; the cached link output had none.
          1 => check_or_none!(!report_cycles, "cycle"),
          _ => {}
        },
        Step::Exit(idx) => {
          state[idx.index()] = 2;
          exec_orders[idx] = next_exec_order;
          next_exec_order += 1;
          order.push(idx);
        }
      }
    }

    let mut last_old_exec_order = None;
    let mut refreshed = Vec::new();
    let mut reordered = false;
    for &idx in &order {
      if idx.index() >= old_len {
        continue;
      }
      let old_exec_order = old_modules[idx].exec_order();
      if old_exec_order == u32::MAX {
        // A module the cached output no longer reached returns as an appended module when the
        // update rescanned it; otherwise it would return without its factory.
        check_or_none!(
          self.cache.dirty_native_modules.contains(&idx) && is_refreshed_candidate(cached, idx),
          "revived"
        );
        refreshed.push(idx);
        continue;
      }
      reordered |= last_old_exec_order.is_some_and(|last| old_exec_order < last);
      last_old_exec_order = Some(old_exec_order);
    }
    if reordered {
      check_or_none!(!modules.iter().any(Module::is_external), "exec_order_external");
      check_or_none!(
        !cached.link_output.metas.iter().any(|meta| matches!(meta.wrap_kind(), WrapKind::Esm)),
        "exec_order_wrapped"
      );
      check_or_none!(cached.generate_state.order_state.is_empty(), "exec_order_order_state");
    }
    check_or_none!(added.iter().all(|idx| exec_orders[*idx] != u32::MAX), "unreachable_added");
    refreshed.sort_unstable();
    let added: Vec<ModuleIdx> = refreshed.into_iter().chain(added.iter().copied()).collect();
    let mut retired = Vec::new();
    for (idx, module) in old_modules.iter_enumerated() {
      if module.exec_order() != u32::MAX && exec_orders[idx] == u32::MAX {
        check_or_none!(module.is_normal(), "retired_external");
        retired.push(idx);
      }
    }
    // A module that the update makes reachable or unreachable is one that it appends or retires,
    // whose records the checks below follow, or one without a record before or after, as one that
    // only an `import.meta.hot.accept()` reaches (`recordless_module`). So is a module that keeps
    // being reached but gains or loses its record.
    let unreachable = reachability.unreachable(modules);
    for (idx, module) in old_modules.iter_enumerated() {
      let flips = cached.link_output.unreachable_modules.reaches(idx) != unreachable.reaches(idx);
      let recordless = exec_orders[idx] == u32::MAX && module.exec_order() == u32::MAX;
      check_or_none!(
        !flips || !recordless && (retired.contains(&idx) || added.contains(&idx)),
        if recordless { "recordless_module" } else { "reachability" },
        module.stable_id().to_string()
      );
      check_or_none!(
        flips || !(retired.contains(&idx) || added.contains(&idx)),
        "recordless_module",
        module.stable_id().to_string()
      );
      if unreachable.reaches(idx)
        && let (Some(old), Some(new)) = (module.as_normal(), modules[idx].as_normal())
        && matches!(new.exports_kind, ExportsKind::None)
        && !new.meta.has_lazy_export()
      {
        check_or_none!(
          linked_exports_kind(snapshot, &unreachable, idx) == old.exports_kind,
          "imported_exports_kind",
          new.stable_id.to_string()
        );
      }
    }
    let first_chunk = cached.generate_state.chunk_graph.chunk_table.len();
    let added_chunks = added
      .iter()
      .enumerate()
      .map(|(position, idx)| (*idx, ChunkIdx::from_usize(first_chunk + position)))
      .collect();
    let sorted_modules = order.into_iter().filter(|idx| modules[*idx].is_normal()).collect();
    Some(NativeGraphPlan {
      added,
      retired,
      exec_orders,
      unreachable,
      sorted_modules,
      reordered,
      added_chunks,
      importer_changes: Vec::new(),
    })
  }

  /// Whether the common directory of the preserved modules' paths, which names every record,
  /// stays the cached one: appended paths lie inside it, and after retirements two paths
  /// still part right below it.
  fn keeps_input_base(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
  ) -> bool {
    let cwd = self.options.cwd.as_path();
    let Some(base) = cached
      .generate_state
      .chunk_graph
      .chunk_table
      .iter()
      .next()
      .map(|chunk| chunk.input_base.clone())
    else {
      return false;
    };
    let modules = &snapshot.module_table.modules;
    for &idx in &plan.added {
      if let Some(path) = common_dir_input(modules[idx].id(), cwd)
        && below_input_base(&path, &base).is_none()
      {
        return false;
      }
    }
    let retired: FxHashSet<ModuleIdx> = plan.retired.iter().copied().collect();
    if !retired.iter().any(|idx| common_dir_input(modules[*idx].id(), cwd).is_some()) {
      return true;
    }
    let old_len = cached.link_output.metas.len();
    let mut first_part = None;
    for (idx, module) in modules.iter_enumerated() {
      let included = if plan.added_chunks.contains_key(&idx) {
        true
      } else if idx.index() < old_len {
        cached.link_output.metas[idx].is_included && !retired.contains(&idx)
      } else {
        false
      };
      if !included || !module.is_normal() {
        continue;
      }
      let Some(path) = common_dir_input(module.id(), cwd) else {
        continue;
      };
      let Some(mut rest) = below_input_base(&path, &base) else {
        return false;
      };
      let part = rest.next().map(|part| part.as_os_str().to_owned());
      match &first_part {
        None => first_part = Some(part),
        Some(first) if *first != part => return true,
        Some(_) => {}
      }
    }
    false
  }

  /// Checks the cached modules whose importer sets the update changed: only modules that the
  /// update appends, retires or edits start or stop importing them, and they keep their linked
  /// export kind. Returns the modules whose importer sets the cached link output copies from
  /// the scan.
  fn check_native_importer_changes(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
  ) -> Option<Vec<ModuleIdx>> {
    let added: FxHashSet<ModuleIdx> = plan.added.iter().copied().collect();
    let retired: FxHashSet<ModuleIdx> = plan.retired.iter().copied().collect();
    let mut changes = Vec::new();
    for &idx in &self.cache.native_importer_changes {
      if idx.index() >= cached.link_output.module_table.modules.len()
        || retired.contains(&idx)
        || plan.exec_orders[idx] == u32::MAX
        || self.cache.dirty_native_modules.contains(&idx)
      {
        continue;
      }
      let old = cached.link_output.module_table.modules[idx].as_normal()?;
      let new = snapshot.module_table.modules[idx].as_normal()?;
      check_or_none!(
        old.ecma_view.dynamic_importers == new.ecma_view.dynamic_importers,
        "importer_dynamic"
      );
      // An appended importer can give a JSON module's namespace its properties, which its
      // cached chunk lacks.
      check_or_none!(
        crate::utils::import_options::json_namespace_has_properties(
          old,
          &cached.link_output.module_table.modules,
          |idx| cached.link_output.unreachable_modules.reaches(idx),
        ) == crate::utils::import_options::json_namespace_has_properties(
          new,
          &snapshot.module_table.modules,
          |idx| plan.unreachable.reaches(idx),
        ),
        "json_namespace_properties"
      );
      let old_importers: FxHashSet<ModuleIdx> =
        old.ecma_view.importers_idx.iter().copied().collect();
      let new_importers: FxHashSet<ModuleIdx> =
        new.ecma_view.importers_idx.iter().copied().collect();
      // The importers that the update appends, retires or edits are linked anew. An edited
      // one's new and dropped records are checked as it is prepared (`new_edge`,
      // `dropped_edge`), so it may start or stop importing a module in the output (#626).
      let relinked = |importer: &ModuleIdx| {
        added.contains(importer)
          || retired.contains(importer)
          || self.cache.dirty_native_modules.contains(importer)
      };
      let name = |idx: ModuleIdx| snapshot.module_table.modules[idx].stable_id().to_string();
      if let Some(&importer) = new_importers
        .iter()
        .find(|importer| !old_importers.contains(importer) && !relinked(importer))
      {
        reject_with!("importer_set", format!("{} imports {}", name(importer), name(idx)));
      }
      if let Some(&importer) = old_importers
        .iter()
        .find(|importer| !new_importers.contains(importer) && !relinked(importer))
      {
        reject_with!("importer_set", format!("{} no longer imports {}", name(importer), name(idx)));
      }
      // The first reached importer in discovery order decides the kind of a module without
      // exports (`determine_module_exports_kind`, #681).
      if matches!(new.exports_kind, ExportsKind::None) && !new.meta.has_lazy_export() {
        check_or_none!(
          linked_exports_kind(snapshot, &plan.unreachable, idx) == old.exports_kind,
          "importer_exports_kind"
        );
      }
      changes.push(idx);
    }
    Some(changes)
  }

  /// Checks that the records of retired modules leave nothing behind: no remaining module
  /// imports them, and every binding they imported stays exported by its record.
  fn check_retired_modules(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
  ) -> Option<()> {
    if plan.retired.is_empty() {
      return Some(());
    }
    let retired: FxHashSet<ModuleIdx> = plan.retired.iter().copied().collect();
    let link_output = &cached.link_output;
    let chunk_graph = &cached.generate_state.chunk_graph;
    let retired_chunks: FxHashSet<ChunkIdx> =
      plan.retired.iter().filter_map(|idx| chunk_graph.module_to_chunk[*idx]).collect();
    let mut needed: FxHashSet<(ChunkIdx, SymbolRef)> = FxHashSet::default();
    // Whether a symbol is used in the cached output: an edit keeps whether a record that stays
    // uses each of its kept named imports (`named_import_used_changed`).
    let cached_used = |symbol: SymbolRef| {
      cached.generate_state.used_overrides.get(&symbol.owner).map_or_else(
        || cached.generate_state.used_symbol_refs.contains(&symbol),
        |used| used.contains(&symbol),
      )
    };
    // The facades of external modules' bindings that retired records import, with whether a
    // retired record uses each, and whether it is a default export's.
    let mut external_facades: FxHashMap<SymbolRef, (bool, bool)> = FxHashMap::default();
    for &idx in &plan.retired {
      // A barrel, a module that re-exports with `export *`, is protected with every module that it
      // re-exports or imports, so that no update changes what it re-exports (#629). An update that
      // retires one leaves no record that stays importing it (`retired_importers`), so none binds
      // a name through it, and the checks below and `keeps_imported_link_data` find whether its
      // records' retirement changes the exports, wrapping, interop or evaluation of the modules
      // that stay. So the barrel retires, as may a module that only retired barrels protected
      // (#705). Native cells, and the modules that chunk decisions depend on, stay refused.
      check_or_none!(
        idx != link_output.runtime.id()
          && !self.cache.native_cell_modules.contains(&idx)
          && !cached.generate_state.decision_dependencies.contains(&idx)
          && !link_output.user_defined_entry_modules.contains(&idx)
          && !link_output.entries.contains_key(&idx),
        "retired_protected",
        link_output.module_table.modules[idx].stable_id().to_string()
      );
      // A barrel whose `export *` reaches a CommonJS or external module has dynamic exports, and
      // what its importers do with them decides which of a CommonJS module's constants linking
      // inlines (#693), which no check of the barrel's own records finds: it stays refused. The
      // cached records tell, whatever the update's scan of the module says.
      let barrel = link_output.module_table.modules[idx].as_normal().is_some_and(|module| {
        module.import_records.iter().any(|rec| rec.meta.contains(ImportRecordMeta::IsExportStar))
      });
      check_or_none!(
        !barrel
          || !link_output.metas[idx].has_dynamic_exports
            && link_output.metas[idx].star_exports_from_external_modules.is_empty(),
        "retired_protected",
        link_output.module_table.modules[idx].stable_id().to_string()
      );
      let module = link_output.module_table.modules[idx].as_normal()?;
      let current = snapshot.module_table.modules[idx].as_normal()?;
      check_or_none!(
        matches!(
          module.module_type,
          ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx
        ) && module.import_records.iter().all(|rec| {
          matches!(rec.kind, ImportKind::Import | ImportKind::HotAccept)
            && rec.dynamic_import_expr_info.is_none()
        }) && module.ecma_view.new_url_references.is_empty()
          && module.ecma_view.rolldown_file_url_references.is_empty(),
        "retired_module"
      );
      check_or_none!(
        current.ecma_view.dynamic_importers.is_empty()
          && current
            .ecma_view
            .importers_idx
            .iter()
            .all(|importer| {
              *importer == idx
                || retired.contains(importer)
                || plan.exec_orders[*importer] == u32::MAX
            }),
        "retired_importers"
      );
      // A wrapped module wraps the ES modules that its records import, which linking the updated
      // graph doesn't do once the graph no longer reaches it (#660).
      let wraps_kept_module = !matches!(link_output.metas[idx].wrap_kind(), WrapKind::None)
        && module.import_records.iter().filter_map(|rec| rec.resolved_module).any(|target| {
          !retired.contains(&target)
            && plan.exec_orders[target] != u32::MAX
            && matches!(link_output.metas[target].wrap_kind(), WrapKind::Esm)
        });
      check_or_none!(!wraps_kept_module, "retired_wrapper", module.stable_id.to_string());
      // Linking inlines none of the constants that a CommonJS module exports once a module lets
      // its exports object escape through an import of it (`resolve_member_expr_refs`). Once the
      // update retires that module, linking inlines them again in every module that reads them,
      // whose records the relink would keep (#693).
      check_or_none!(
        !escapes_kept_commonjs_exports(link_output, snapshot, idx, |target| {
          !retired.contains(&target) && plan.exec_orders[target] != u32::MAX
        }),
        "retired_commonjs_escape",
        module.stable_id.to_string()
      );
      // A CommonJS module's interop is shared by its importers' records: another record that
      // the updated graph keeps must still need it, unless none imports the module statically
      // any more, which leaves it no interop (`release_retired_interop`, #684).
      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        let Some(target) = rec.resolved_module else {
          continue;
        };
        if rec.kind != ImportKind::Import
          || rec.meta.contains(ImportRecordMeta::IsExportStar)
          || !link_output.module_table.modules[target]
            .as_normal()
            .is_some_and(|target| target.exports_kind.is_commonjs())
          || !import_record_needs_interop(module, rec_idx)
        {
          continue;
        }
        let (mut imported, mut needed) = (false, false);
        if let Some(target_module) = snapshot.module_table.modules[target].as_normal() {
          for &importer in &target_module.ecma_view.importers_idx {
            // Linking counts the records of every module that the graph reaches, also of one that
            // only an `import.meta.hot.accept()` reaches, which has no record of its own
            // (`determine_safely_merge_cjs_ns`), as `release_retired_interop` does.
            if retired.contains(&importer) || !plan.unreachable.reaches(importer) {
              continue;
            }
            let Some(importer) = snapshot.module_table.modules[importer].as_normal() else {
              continue;
            };
            for (rec_idx, rec) in importer.import_records.iter_enumerated() {
              if rec.kind == ImportKind::Import
                && !rec.meta.contains(ImportRecordMeta::IsExportStar)
                && rec.resolved_module == Some(target)
              {
                imported = true;
                needed |= import_record_needs_interop(importer, rec_idx);
              }
            }
          }
        }
        check_or_none!(needed || !imported, "retired_interop");
      }
      // Linking merges the named and default imports of an external module, by the name that
      // they import, into one facade (`bind_imports_and_exports`), whose use is the output's: a
      // record renders its specifier of the binding only while some record uses the facade, and
      // every chunk that imports the module reserves the facade's name while one does. It names
      // the facade of the default export after the import that comes first in execution order.
      for (&symbol, import) in &module.named_imports {
        let Some(target) = module.import_records[import.record_idx].resolved_module else {
          continue;
        };
        let facade = link_output.symbol_db.canonical_ref_for(symbol);
        // An import of a name that an empty module doesn't export makes linking shim the name in
        // the module's record (`shim_missing_export`), which a whole link of the updated graph
        // drops with the last such import.
        check_or_none!(
          retired.contains(&facade.owner) || !is_shimmed_export(link_output, facade),
          "retired_shim",
          link_output.module_table.modules[facade.owner].stable_id().to_string()
        );
        if facade.owner == target && link_output.module_table.modules[target].is_external() {
          let (used, default) = external_facades.entry(facade).or_default();
          *used |= cached_used(symbol);
          *default |=
            matches!(&import.imported, Specifier::Literal(name) if name.as_str() == "default");
        }
      }
      let Some(chunk_idx) = chunk_graph.module_to_chunk[idx] else {
        continue;
      };
      for (exporter, items) in &chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks {
        if retired_chunks.contains(exporter) {
          continue;
        }
        let signature = entry_signature(cached, *exporter);
        for item in items {
          if !signature.contains(&item.import_ref) {
            needed.insert((*exporter, item.import_ref));
          }
        }
      }
    }
    // A binding that only retired records imported would leave its exporter's record.
    if !needed.is_empty() {
      for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
        if retired_chunks.contains(&chunk_idx) || !is_live_chunk(chunk_graph, chunk_idx) {
          continue;
        }
        for (exporter, items) in &chunk.imports_from_other_chunks {
          for item in items {
            needed.remove(&(*exporter, item.import_ref));
          }
        }
        if needed.is_empty() {
          break;
        }
      }
      check_or_none!(needed.is_empty(), "retired_exports");
    }
    // Once the update retires the imports of such a facade, linking the updated graph derives it
    // from the imports of the modules that stay, whose records the relink keeps: a facade that a
    // retired record used must stay used by one of them, and the default export's must keep its
    // name, which linking takes from the next import. Those modules keep their order, as an update
    // that reorders the graph of an external module links the whole graph
    // (`exec_order_external`), and the names of their imports (`external_default_name`).
    for (facade, (used, default)) in external_facades {
      let survivors = link_output
        .module_table
        .modules
        .iter()
        .filter(|module| {
          module.exec_order() != u32::MAX
            && !retired.contains(&module.idx())
            && plan.exec_orders[module.idx()] != u32::MAX
        })
        .filter_map(|module| Some((module.exec_order(), module.as_normal()?)))
        .flat_map(|(exec_order, module)| {
          module.named_imports.iter().filter_map(move |(&symbol, import)| {
            (module.import_records[import.record_idx].resolved_module == Some(facade.owner)
              && link_output.symbol_db.canonical_ref_for(symbol) == facade)
              .then_some((exec_order, symbol))
          })
        })
        .collect::<Vec<_>>();
      let external = || link_output.module_table.modules[facade.owner].stable_id().to_string();
      check_or_none!(
        !used || survivors.iter().any(|&(_, symbol)| cached_used(symbol)),
        "retired_external_use",
        external()
      );
      let next = survivors
        .iter()
        .map(|&(exec_order, symbol)| (exec_order, symbol.name(&link_output.symbol_db)))
        .min();
      check_or_none!(
        !default || next.is_none_or(|(_, name)| name == facade.name(&link_output.symbol_db)),
        "retired_external_default",
        external()
      );
    }
    Some(())
  }

  /// Checks that the records of the modules that the update appends or retires leave the link
  /// data of the modules that they import as it is, as linking derives it from the modules that
  /// the graph reaches (#660), and the relink keeps it. The first `import` or `require` of a module
  /// without exports makes it an ES or a CommonJS module, an `import()` or
  /// `import.meta.hot.accept()` makes it `ChunkMeta::DynamicImported`, and an importer that isn't
  /// strict ESM gives a JSON module's namespace its properties (`json_namespace_has_properties`).
  /// A CommonJS module's record calls its wrapper as it evaluates while a static `import` or one
  /// of those imports it, and on its first `require` otherwise (`render_wrapped_entry_chunk`):
  /// returns the modules whose records the update makes start or stop calling it, which the relink
  /// renders again (#684). An edit that adds or drops a record of such a module links the whole
  /// graph already (`edge_target`), as does one of a module without a record
  /// (`recordless_module`).
  fn keeps_imported_link_data(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
  ) -> Option<Vec<ModuleIdx>> {
    let link_output = &cached.link_output;
    let modules = &link_output.module_table.modules;
    let scanned_modules = &snapshot.module_table.modules;
    let importers = plan
      .retired
      .iter()
      .filter_map(|&idx| modules[idx].as_normal())
      .chain(plan.added.iter().filter_map(|&idx| scanned_modules[idx].as_normal()))
      .collect::<Vec<_>>();
    let records_of = |module: &rolldown_common::NormalModule| {
      module.import_records.iter().filter_map(|rec| rec.resolved_module).collect::<Vec<_>>()
    };
    let mut imported = importers.iter().flat_map(|&module| records_of(module)).collect::<Vec<_>>();
    imported.sort_unstable();
    imported.dedup();
    let mut targets = imported
      .iter()
      .copied()
      .filter(|&target| {
        target.index() < modules.len()
          && plan.exec_orders[target] != u32::MAX
          && !plan.added_chunks.contains_key(&target)
          && !self.cache.dirty_native_modules.contains(&target)
      })
      .collect::<Vec<_>>();
    targets.sort_unstable();
    targets.dedup();
    // The wrapped CommonJS modules whose records call their wrappers as they evaluate only while
    // a static import of a module that the graph reaches imports them, and whose static importers
    // the update gives a first one or retires the last of: the relink renders their records again,
    // whose link data the update leaves as they are (#684).
    let mut flipped = Vec::new();
    for target in targets {
      let (Some(old), Some(new)) =
        (modules[target].as_normal(), scanned_modules[target].as_normal())
      else {
        continue;
      };
      if matches!(new.exports_kind, ExportsKind::None) && !new.meta.has_lazy_export() {
        check_or_none!(
          linked_exports_kind(snapshot, &plan.unreachable, target) == old.exports_kind,
          "imported_exports_kind",
          new.stable_id.to_string()
        );
      }
      let dynamically_imported = plan.unreachable.dynamically_imported(new);
      check_or_none!(
        link_output.unreachable_modules.dynamically_imported(old) == dynamically_imported,
        "dynamically_imported",
        new.stable_id.to_string()
      );
      if matches!(link_output.metas[target].wrap_kind(), WrapKind::Cjs)
        && !link_output.user_defined_entry_modules.contains(&target)
        && link_output.unreachable_modules.imported_by_static_or_dynamic_import(modules, target)
          != plan.unreachable.imported_by_static_or_dynamic_import(scanned_modules, target)
      {
        flipped.push(target);
      }
    }
    // A JSON module's namespace and its strict importers' reads of it follow its importers that
    // the graph reaches, also those behind the proxy of a lazy `import()`, which stands for them,
    // and also once the update edits the JSON module.
    let mut json_modules = imported
      .iter()
      .copied()
      .flat_map(|target| {
        let proxy = scanned_modules
          .get(target)
          .and_then(Module::as_normal)
          .filter(|module| crate::utils::import_options::is_lazy_proxy(module));
        std::iter::once(target).chain(proxy.into_iter().flat_map(records_of))
      })
      .filter(|&target| target.index() < modules.len() && plan.exec_orders[target] != u32::MAX)
      .collect::<Vec<_>>();
    json_modules.sort_unstable();
    json_modules.dedup();
    for target in json_modules {
      let (Some(old), Some(new)) =
        (modules[target].as_normal(), scanned_modules[target].as_normal())
      else {
        continue;
      };
      check_or_none!(
        crate::utils::import_options::json_namespace_has_properties(old, modules, |idx| {
          link_output.unreachable_modules.reaches(idx)
        }) == crate::utils::import_options::json_namespace_has_properties(
          new,
          scanned_modules,
          |idx| plan.unreachable.reaches(idx),
        ),
        "json_namespace_properties",
        new.stable_id.to_string()
      );
    }
    Some(flipped)
  }

  /// Drops the interop of each CommonJS module that a retired module imported statically and
  /// that no record that the updated graph reaches imports so any more, but through an
  /// `export *`, since `determine_safely_merge_cjs_ns` merges none for it then (#684).
  fn release_retired_interop(
    &self,
    cached: &mut CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
  ) {
    let imports_statically = |importer: ModuleIdx, target: ModuleIdx| {
      snapshot.module_table.modules[importer].as_normal().is_some_and(|importer| {
        importer.import_records.iter().any(|rec| {
          rec.kind == ImportKind::Import
            && !rec.meta.contains(ImportRecordMeta::IsExportStar)
            && rec.resolved_module == Some(target)
        })
      })
    };
    let mut released = plan
      .retired
      .iter()
      .flat_map(|&idx| {
        [
          cached.link_output.module_table.modules.get(idx),
          snapshot.module_table.modules.get(idx),
        ]
      })
      .filter_map(|module| module.and_then(Module::as_normal))
      .flat_map(|module| module.import_records.iter().filter_map(|rec| rec.resolved_module))
      .filter(|target| cached.link_output.safely_merge_cjs_ns_map.contains_key(target))
      .filter(|&target| {
        snapshot.module_table.modules[target].as_normal().is_some_and(|module| {
          !module.ecma_view.importers_idx.iter().any(|&importer| {
            plan.unreachable.reaches(importer) && imports_statically(importer, target)
          })
        })
      })
      .collect::<Vec<_>>();
    released.sort_unstable();
    released.dedup();
    for target in released {
      cached.link_output.safely_merge_cjs_ns_map.remove(&target);
    }
  }

  /// Whether linking the updated graph includes every module that the fast relink keeps a record
  /// for: each appended module, and each module that a retired module imported and the cached
  /// output includes. Linking includes a module through a record of an included importer
  /// (`include_side_effectful_dependencies`) rather than through reachability: development skips
  /// a side-effect-free module that only unused imports reach.
  fn keeps_inclusion(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
  ) -> bool {
    let retired: FxHashSet<ModuleIdx> = plan.retired.iter().copied().collect();
    let link_output = &cached.link_output;
    let mut candidates = plan.added.clone();
    for &idx in &plan.retired {
      let Some(module) = link_output.module_table.modules[idx].as_normal() else {
        continue;
      };
      candidates.extend(module.import_records.iter().filter_map(|rec| rec.resolved_module).filter(
        |target| {
          !retired.contains(target)
            && plan.exec_orders[*target] != u32::MAX
            && link_output.module_table.modules[*target].is_normal()
            && link_output.metas[*target].is_included
        },
      ));
    }
    // A module in the output that an edited module stops importing must stay included (#626).
    let added: FxHashSet<ModuleIdx> = plan.added.iter().copied().collect();
    for &idx in &self.cache.dirty_native_modules {
      if added.contains(&idx) || retired.contains(&idx) {
        continue;
      }
      let (Some(old), Some(new)) = (
        link_output.module_table.modules.get(idx).and_then(Module::as_normal),
        snapshot.module_table.modules.get(idx).and_then(Module::as_normal),
      ) else {
        continue;
      };
      let new_targets: FxHashSet<ModuleIdx> =
        new.import_records.iter().filter_map(|rec| rec.resolved_module).collect();
      candidates.extend(old.import_records.iter().filter_map(|rec| rec.resolved_module).filter(
        |target| {
          !new_targets.contains(target)
            && !retired.contains(target)
            && plan.exec_orders[*target] != u32::MAX
            && link_output.module_table.modules[*target].is_normal()
            && link_output.metas[*target].is_included
        },
      ));
    }
    candidates.sort_unstable();
    candidates.dedup();
    let mut included: FxHashSet<ModuleIdx> =
      link_output.user_defined_entry_modules.iter().copied().collect();
    candidates
      .into_iter()
      .all(|idx| self.reaches_entry(cached, snapshot, plan, &retired, &mut included, idx))
  }

  /// Whether a chain of records that linking includes leads to `start` from a user-defined entry
  /// through modules that the updated graph reaches and the cached output includes. `included`
  /// collects the modules proven so.
  fn reaches_entry(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
    retired: &FxHashSet<ModuleIdx>,
    included: &mut FxHashSet<ModuleIdx>,
    start: ModuleIdx,
  ) -> bool {
    if included.contains(&start) {
      return true;
    }
    let modules = &snapshot.module_table.modules;
    // The path from `start` up its importers, each with the next of its importers to try.
    let mut path = vec![(start, 0usize)];
    let mut visited = FxHashSet::from_iter([start]);
    while let Some(&(target, position)) = path.last() {
      let mut next = position;
      let mut found = None;
      if let Some(module) = modules[target].as_normal() {
        let static_importers = &module.ecma_view.importers_idx;
        let dynamic_importers = &module.ecma_view.dynamic_importers_idx;
        while next < static_importers.len() + dynamic_importers.len() {
          let importer = if next < static_importers.len() {
            static_importers[next]
          } else {
            dynamic_importers[next - static_importers.len()]
          };
          next += 1;
          if importer != target
            && !retired.contains(&importer)
            && plan.exec_orders[importer] != u32::MAX
            && cached.link_output.metas[importer].is_included
            && !visited.contains(&importer)
            && self.includes_through(cached, snapshot, plan, importer, target)
          {
            found = Some(importer);
            break;
          }
        }
      }
      let Some(importer) = found else {
        path.pop();
        continue;
      };
      if let Some(top) = path.last_mut() {
        top.1 = next;
      }
      if included.contains(&importer) {
        included.extend(path.iter().map(|(module, _)| *module));
        return true;
      }
      visited.insert(importer);
      path.push((importer, 0));
    }
    false
  }

  /// Whether `importer`'s records make linking include `target` once it includes `importer`: a
  /// static record evaluates or binds it, unless in development every record is an unused import
  /// of a side-effect-free module, and a dynamic import that linking keeps makes it an entry.
  fn includes_through(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
    importer: ModuleIdx,
    target: ModuleIdx,
  ) -> bool {
    // Changed and appended modules link their scans. The others' cached link data holds their
    // scope trees and marks the dynamic imports that linking found dead.
    let rescanned = self.cache.dirty_native_modules.contains(&importer)
      || plan.added_chunks.contains_key(&importer);
    let (module, db) = if rescanned {
      (
        snapshot.module_table.modules[importer].as_normal(),
        snapshot.symbol_ref_db[importer].as_ref(),
      )
    } else {
      (
        cached.link_output.module_table.modules[importer].as_normal(),
        cached.link_output.symbol_db[importer].as_ref(),
      )
    };
    let (Some(module), Some(db)) = (module, db) else {
      return false;
    };
    let modules = &cached.link_output.module_table.modules;
    let records = module
      .import_records
      .iter_enumerated()
      .filter(|(_, rec)| rec.resolved_module == Some(target))
      .collect::<Vec<_>>();
    if records.iter().any(|(_, rec)| {
      rec.kind.is_dynamic() && !rec.meta.contains(ImportRecordMeta::DeadDynamicImport)
    }) {
      return true;
    }
    records.iter().any(|(_, rec)| rec.kind.is_static())
      && !(self.options.is_dev_mode_enabled()
        && !modules[target].side_effects().has_side_effects()
        && records.iter().all(|(rec_idx, _)| {
          crate::hmr::utils::unused_side_effect_free_import(
            module,
            *rec_idx,
            modules,
            db.ast_scopes.scoping(),
          )
        }))
  }

  /// Appends the modules that partial scans added to the cached link output, with the link
  /// data that linking the whole graph gives them. Their scope trees stay in the scan snapshot
  /// until the relink applies.
  fn seed_added_modules(
    &self,
    cached: &mut CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    plan: &NativeGraphPlan,
  ) -> Option<()> {
    if plan.added.is_empty() {
      return Some(());
    }
    let added: FxHashSet<ModuleIdx> = plan.added.iter().copied().collect();
    let retired: FxHashSet<ModuleIdx> = plan.retired.iter().copied().collect();
    let old_len = cached.link_output.module_table.modules.len();
    // Whether a target that a record of an appended module reaches is a CommonJS module once
    // seeded: appended ones keep the exports kind of their scan.
    let is_commonjs_target = |target: ModuleIdx| {
      let module = if added.contains(&target) {
        snapshot.module_table.modules.get(target)
      } else {
        cached.link_output.module_table.modules.get(target)
      };
      module.and_then(Module::as_normal).is_some_and(|module| module.exports_kind.is_commonjs())
    };
    // Cached CommonJS modules that the appended records import first: no record that the graph
    // reaches imports one statically but through an `export *`, which merges into no interop.
    let mut first_interop = FxHashSet::default();
    for &idx in &plan.added {
      let module = snapshot.module_table.modules[idx].as_normal()?;
      // Each check below names the module that it refuses.
      macro_rules! check_module {
        ($cond:expr, $label:expr) => {
          check_or_none!($cond, $label, module.stable_id.to_string())
        };
      }
      check_module!(
        matches!(
          module.module_type,
          ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx
        ),
        "added_module_type"
      );
      // An appended CommonJS module gets the `require_*` wrapper that linking the whole graph
      // gives it (#653). One that returns to the output keeps its retirement's link data.
      let is_commonjs = module.exports_kind.is_commonjs();
      check_module!(
        !is_commonjs || idx.index() >= old_len && !self.options.is_strict_execution_order_enabled(),
        "added_module_commonjs"
      );
      check_module!(
        !crate::utils::native_records::is_native_cell(module.id.as_str())
          && !self.cache.native_cell_modules.contains(&idx),
        "added_module_native_cell"
      );
      // A barrel's `export *` records re-export what their targets export, which the relink
      // resolves below once every appended module is in the table (#629).
      check_module!(
        module.import_records.iter().all(|rec| {
          !rec.meta.contains(ImportRecordMeta::IsExportStar) || rec.resolved_module.is_some()
        }),
        "added_module_export_star"
      );
      check_module!(
        !module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait)
          && !snapshot.tla_keyword_span_map.contains_key(&idx),
        "added_module_top_level_await"
      );
      check_module!(
        is_commonjs
          || !module
            .ast_usage
            .intersects(EcmaModuleAstUsage::IsCjsReexport | EcmaModuleAstUsage::ModuleOrExports)
            && module.ecma_view.cjs_reexport_import_record_ids.is_empty(),
        "added_module_commonjs_syntax"
      );
      check_module!(!module.meta.contains(EcmaViewMeta::Eval), "added_module_eval");
      check_module!(!module.meta.contains(EcmaViewMeta::HasLazyExport), "added_module_lazy_export");
      check_module!(
        module.ecma_view.dynamic_importers.is_empty(),
        "added_module_dynamic_importers"
      );
      check_module!(module.ecma_view.enum_member_value_map.is_empty(), "added_module_enums");
      check_module!(module.ecma_view.mutations.is_empty(), "added_module_mutations");
      check_module!(
        module.ecma_view.json_module_none_self_reference_included_symbol.is_none(),
        "added_module_json"
      );
      check_module!(module.ecma_view.dummy_record_set.is_empty(), "added_module_dummy_records");
      check_module!(
        module.ecma_view.rolldown_file_url_references.is_empty()
          && module.ecma_view.new_url_references.is_empty(),
        "added_module_urls"
      );
      check_module!(
        module.ecma_view.import_attribute_map.is_empty(),
        "added_module_import_attributes"
      );
      check_module!(module.ecma_view.hashbang_range.is_none(), "added_module_hashbang");
      // An export of an imported binding forwards an ES module's export, which binding the
      // module's imports links to, as linking the whole graph does (#649), or a CommonJS module's,
      // which the binding reads from the record's interop namespace (#674). A forward of an
      // external module's binding would read the importer's external interop. A CommonJS
      // module's exports are its own assignments to `exports`.
      check_module!(
        module.named_exports.values().all(|export| {
          export.came_from_commonjs == is_commonjs
            && export.referenced.owner == idx
            && module.named_imports.get(&export.referenced).is_none_or(|import| {
              module.import_records[import.record_idx].resolved_module.is_some_and(|target| {
                if added.contains(&target) {
                  snapshot.module_table.modules[target].is_normal()
                } else {
                  cached.link_output.module_table.modules.get(target).is_some_and(|target| {
                    target.as_normal().is_some_and(|target| {
                      matches!(target.exports_kind, ExportsKind::Esm | ExportsKind::CommonJs)
                    })
                  })
                }
              })
            })
        }),
        "added_exports"
      );
      check_module!(
        is_commonjs
          || matches!(module.exports_kind, ExportsKind::Esm)
          || (!module.meta.has_lazy_export()
            && matches!(
              linked_exports_kind(snapshot, &plan.unreachable, idx),
              ExportsKind::Esm
            )),
        "added_exports_kind"
      );
      // Linking drops every constant that a CommonJS module exports once an importer lets its
      // exports object escape (#693).
      check_module!(
        !writes_commonjs_exports(
          module,
          snapshot.symbol_ref_db[idx].as_ref()?,
          |_| true,
          |target, named| escape_drops_constants(snapshot, target, named)
        ),
        "cjs_computed_write"
      );
      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        check_or_none!(
          (matches!(rec.kind, ImportKind::Import | ImportKind::HotAccept)
            || is_commonjs && rec.kind == ImportKind::Require)
            && rec.dynamic_import_expr_info.is_none()
            && rec.module_request != "\0rolldown/computed-import",
          "added_record",
          format!("{} imports {}", module.stable_id, rec.module_request)
        );
        // Linking wraps every module that a wrapped module's records reach, so an appended
        // CommonJS module may only require CommonJS modules, which keep their wrapper and exports
        // kind: an appended one is seeded with its wrapper, and the cached link output already
        // wraps whatever a cached one reaches (#653).
        check_or_none!(
          !is_commonjs
            || rec.kind == ImportKind::Require
              && rec.resolved_module.is_some_and(|target| {
                is_commonjs_target(target)
                  && (added.contains(&target)
                    || matches!(cached.link_output.metas[target].wrap_kind(), WrapKind::Cjs))
              }),
          "added_commonjs_target",
          format!(
            "{} {} {}",
            module.stable_id,
            if rec.kind == ImportKind::Require { "requires" } else { "imports" },
            rec.resolved_module.map_or_else(
              || rec.module_request.to_string(),
              |target| snapshot.module_table.modules[target].stable_id().to_string()
            )
          )
        );
        let target_idx = rec.resolved_module?;
        // An `export *` of a CommonJS or external module gives the barrel dynamic exports.
        let star = rec.meta.contains(ImportRecordMeta::IsExportStar);
        check_or_none!(
          !star
            || snapshot.module_table.modules[target_idx]
              .as_normal()
              .is_some_and(|target| !target.exports_kind.is_commonjs()),
          "added_star_target",
          format!(
            "{} re-exports {}",
            module.stable_id,
            snapshot.module_table.modules[target_idx].stable_id()
          )
        );
        // The generate stage protects every barrel that re-exports with `export *` and every
        // module that one re-exports or imports, so that no update changes what the barrel
        // re-exports. The appended module binds its names through their exports, which the
        // update leaves as they are, and doesn't change how they evaluate (#621). Native cells,
        // and deferred imports' targets and native cells' evaluation dependencies, which chunk
        // decisions assume to evaluate as they did, stay refused.
        let barrel = self.cache.export_star_modules.contains(&target_idx);
        let refused = if retired.contains(&target_idx) {
          Some("retired")
        } else if plan.exec_orders[target_idx] == u32::MAX {
          Some("unordered")
        } else if self.cache.native_cell_modules.contains(&target_idx) {
          Some("a native cell")
        } else if cached.generate_state.decision_dependencies.contains(&target_idx) {
          Some("a chunk decision's dependency")
        } else {
          None
        };
        if let Some(refused) = refused {
          reject_with!(
            "added_target",
            format!(
              "{} imports {}, {refused}",
              module.stable_id,
              snapshot.module_table.modules[target_idx].stable_id()
            )
          );
        }
        if added.contains(&target_idx) {
          continue;
        }
        let target = cached.link_output.module_table.modules[target_idx].as_normal()?;
        let target_meta = &cached.link_output.metas[target_idx];
        let detail = || format!("{} imports {}", module.stable_id, target.stable_id);
        check_or_none!(target_meta.is_included, "added_target_included", detail());
        match target_meta.wrap_kind() {
          // A wrapped ES module's importers call its initializer first, which the relink
          // references as linking the whole graph does (`prepare_native_module`).
          WrapKind::None | WrapKind::Esm => check_or_none!(
            !target.exports_kind.is_commonjs()
              && !target_meta.has_dynamic_exports
              && target_meta.star_exports_from_external_modules.is_empty()
              && (barrel
                || !target.meta.contains(EcmaViewMeta::HasStarExport)
                  && !self.cache.export_star_modules.contains(&target_idx)),
            "added_target_esm",
            detail()
          ),
          WrapKind::Cjs => {
            check_or_none!(
              target.exports_kind.is_commonjs() && !star,
              "added_target_cjs",
              detail()
            );
            if rec.kind == ImportKind::Import {
              // Linking the whole graph merges this record into the module's interop, which
              // strict execution order keeps per record (`determine_safely_merge_cjs_ns`). A module
              // that no module that the graph reaches imports statically, but through an
              // `export *`, has none yet (#660): the appended records that import it give it its
              // first, as linking merges it from every record that imports it, all of them
              // appended ones, since an edit that adds such a record links the whole graph
              // (`edge_target`). Unless an `import()`, an entry or an `export *` already makes the
              // module's cached record call its wrapper as it evaluates, linking then does too
              // (`render_wrapped_entry_chunk`), so the relink renders that record again
              // (`keeps_imported_link_data`, #684).
              check_or_none!(
                !self.options.is_strict_execution_order_enabled(),
                "added_interop_strict_order",
                detail()
              );
              match cached.link_output.safely_merge_cjs_ns_map.get(&target_idx) {
                Some(info) => check_or_none!(
                  info.needs_interop || !import_record_needs_interop(module, rec_idx),
                  "added_interop",
                  detail()
                ),
                None => {
                  first_interop.insert(target_idx);
                }
              }
            }
          }
        }
      }
    }

    let link_output = &mut cached.link_output;
    for &idx in &plan.added {
      let mut seeded = snapshot.module_table.modules[idx].clone();
      let module = seeded.as_normal_mut()?;
      module.exec_order = plan.exec_orders[idx];
      let is_commonjs = module.exports_kind.is_commonjs();
      if !is_commonjs {
        module.exports_kind = ExportsKind::Esm;
      }
      let constants = std::mem::take(&mut module.constant_export_map);
      // `LinkStage::new` gathers the constants that importers read through a CommonJS module's
      // exports; the relink refuses any write that would drop one. Linking drops those of a module
      // whose exports code outside the graph may reach (`commonjs_exports_public`).
      if is_commonjs
        && self.options.optimization.is_inline_const_enabled()
        && !crate::stages::link_stage::commonjs_exports_public(
          &self.options,
          &snapshot.user_defined_entry_modules,
          idx,
        )
      {
        link_output
          .global_constant_symbol_map
          .extend(constants.into_iter().map(|(symbol, value)| ((idx, symbol).into(), value)));
      }

      let mut meta = LinkingMetadata::default();
      meta.dependencies = module
        .import_records
        .iter()
        .filter_map(|rec| match rec.kind {
          ImportKind::DynamicImport | ImportKind::Require | ImportKind::HotAccept => None,
          _ => rec.resolved_module,
        })
        .collect();
      meta.resolved_exports = module
        .named_exports
        .iter()
        .map(|(name, local)| {
          (name.clone(), ResolvedExport::new(local.referenced, local.came_from_commonjs))
        })
        .collect();
      let mut sorted_exports = meta
        .resolved_exports
        .iter()
        .map(|(name, export)| (name.clone(), export.came_from_commonjs))
        .collect::<Vec<_>>();
      sorted_exports.sort_unstable();
      meta.sorted_and_non_ambiguous_resolved_exports = FxIndexMap::from_iter(sorted_exports);
      meta.is_included = true;
      let scanned_db = snapshot.symbol_ref_db[idx].as_ref()?;
      if is_commonjs {
        // `create_wrapper` declares the `require_*` wrapper after the module's scanned symbols
        // and statements; the scope tree gains it as the relink applies.
        check_or_none!(
          scanned_db.ast_scopes.total_symbol_count() == scanned_db.classic_data.len(),
          "added_commonjs_wrapper",
          module.stable_id.to_string()
        );
        meta.set_wrap_kind(WrapKind::Cjs);
        meta.wrapper_ref =
          Some((idx, oxc::semantic::SymbolId::from_usize(scanned_db.classic_data.len())).into());
        meta.wrapper_stmt_info = Some(StmtInfoIdx::from_usize(snapshot.stmt_infos[idx].len()));
      } else {
        // Development output includes every ES module's namespace, which its native record
        // exposes.
        meta.module_namespace_included_reason = ModuleNamespaceIncludedReason::Unknown;
        meta.namespace_included = true;
      }
      let imports_commonjs = |target: Option<ModuleIdx>| {
        target.is_some_and(|target| {
          let module = if added.contains(&target) {
            snapshot.module_table.modules.get(target)
          } else {
            link_output.module_table.modules.get(target)
          };
          module.and_then(Module::as_normal).is_some_and(|module| module.exports_kind.is_commonjs())
        })
      };
      for named_import in module.named_imports.values() {
        let target = module.import_records[named_import.record_idx].resolved_module;
        if imports_commonjs(target) {
          meta.named_import_to_cjs_module.insert(named_import.imported_as, target?);
        }
      }
      for rec in &module.import_records {
        if imports_commonjs(rec.resolved_module) {
          meta.import_record_ns_to_cjs_module.insert(rec.namespace_ref, rec.resolved_module?);
        }
      }

      let mut local_db = SymbolRefDbForModule::new(
        oxc::semantic::Scoping::default(),
        idx,
        oxc::semantic::ScopeId::new(0),
      );
      local_db.classic_data = scanned_db.classic_data.clone();
      local_db.flags = scanned_db.flags.clone();
      if let Some(wrapper) = meta.wrapper_ref {
        local_db.classic_data.push(Default::default());
        local_db.flags.entry(wrapper.symbol).or_default().insert(SymbolRefFlags::IsFacade);
      }

      if idx.index() < link_output.module_table.modules.len() {
        // A refreshed module replaces the link data it kept since the update that retired it.
        link_output.module_table.modules[idx] = seeded;
        link_output.metas[idx] = meta;
        link_output.symbol_db.store_local_db(idx, local_db);
        link_output.stmt_infos[idx] = snapshot.stmt_infos[idx].clone();
        cached.generate_state.used_overrides.remove(&idx);
      } else {
        link_output.module_table.modules.push(seeded);
        link_output.metas.push(meta);
        link_output.symbol_db.store_local_db(idx, local_db);
        link_output.stmt_infos.push(snapshot.stmt_infos[idx].clone());
      }
    }

    // A barrel re-exports the names that its `export *` records' targets export, cached or
    // appended, as linking the whole graph resolves them. A name that two targets export as
    // different symbols needs the bindings' canonical symbols to tell whether it's ambiguous, so
    // it links the whole graph (#629). So does a name of two modules that a CommonJS module
    // assigns to `module.exports` (#653).
    for &idx in &plan.added {
      let Some(module) = link_output.module_table.modules[idx].as_normal() else {
        continue;
      };
      let is_commonjs = module.exports_kind.is_commonjs();
      if !module.has_star_export() && !module.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport)
      {
        continue;
      }
      let stable_id = module.stable_id.to_string();
      let (resolved_exports, star_export_record_by_name) =
        crate::stages::link_stage::appended_module_exports(link_output, &self.options, idx)?;
      check_or_none!(
        resolved_exports.values().all(|export| {
          export.came_from_commonjs == is_commonjs
            && export.potentially_ambiguous_symbol_refs.is_none()
            && export.cjs_conflicting_symbol_refs.is_none()
        }),
        "added_star_ambiguous",
        stable_id
      );
      let mut sorted_exports = resolved_exports
        .iter()
        .map(|(name, export)| (name.clone(), export.came_from_commonjs))
        .collect::<Vec<_>>();
      sorted_exports.sort_unstable();
      let meta = &mut link_output.metas[idx];
      meta.sorted_and_non_ambiguous_resolved_exports = FxIndexMap::from_iter(sorted_exports);
      meta.resolved_exports = resolved_exports;
      meta.star_export_record_by_name = star_export_record_by_name;
    }

    // An appended CommonJS module's importers, all relinked, share its interop, as
    // `determine_safely_merge_cjs_ns` merges it, and what it requires is required by another
    // module, as `wrap_modules` marks it.
    for &idx in &plan.added {
      let Some(module) = snapshot.module_table.modules[idx].as_normal() else {
        continue;
      };
      if !module.exports_kind.is_commonjs() {
        continue;
      }
      for rec in &module.import_records {
        if rec.kind == ImportKind::Require
          && let Some(target) = rec.resolved_module
        {
          link_output.metas[target].required_by_other_module = true;
        }
      }
      let mut importers = module
        .ecma_view
        .importers_idx
        .iter()
        .copied()
        .filter(|importer| plan.unreachable.reaches(*importer))
        .collect::<Vec<_>>();
      importers.sort_unstable();
      let mut interop = crate::stages::link_stage::SafelyMergeCjsNsInfo::default();
      for importer in importers {
        let Some(importer) = snapshot.module_table.modules[importer].as_normal() else {
          continue;
        };
        for (rec_idx, rec) in importer.import_records.iter_enumerated() {
          if rec.kind == ImportKind::Import
            && !rec.meta.contains(ImportRecordMeta::IsExportStar)
            && rec.resolved_module == Some(idx)
          {
            interop.namespace_refs.push(rec.namespace_ref);
            interop.needs_interop |= import_record_needs_interop(importer, rec_idx);
          }
        }
      }
      if !interop.namespace_refs.is_empty() {
        link_output.safely_merge_cjs_ns_map.insert(idx, interop);
      }
    }
    // A cached CommonJS module that appended records import first gets the interop that they
    // merge into, as `determine_safely_merge_cjs_ns` merges it from the records of every module
    // that the graph reaches (#684).
    for &target in &first_interop {
      let module = snapshot.module_table.modules[target].as_normal()?;
      let mut importers = module
        .ecma_view
        .importers_idx
        .iter()
        .copied()
        .filter(|importer| plan.unreachable.reaches(*importer))
        .collect::<Vec<_>>();
      importers.sort_unstable();
      importers.dedup();
      let mut interop = crate::stages::link_stage::SafelyMergeCjsNsInfo::default();
      for importer in importers {
        let Some(importer) = snapshot.module_table.modules[importer].as_normal() else {
          continue;
        };
        for (rec_idx, rec) in importer.import_records.iter_enumerated() {
          if rec.kind == ImportKind::Import
            && !rec.meta.contains(ImportRecordMeta::IsExportStar)
            && rec.resolved_module == Some(target)
          {
            check_or_none!(
              added.contains(&importer.idx),
              "commonjs_first_static_import",
              module.stable_id.to_string()
            );
            interop.namespace_refs.push(rec.namespace_ref);
            interop.needs_interop |= import_record_needs_interop(importer, rec_idx);
          }
        }
      }
      if !interop.namespace_refs.is_empty() {
        link_output.safely_merge_cjs_ns_map.insert(target, interop);
      }
    }

    // A side-effect-free module evaluates its dependencies' side effects.
    let mut by_exec_order = plan.added.clone();
    by_exec_order.sort_unstable_by_key(|idx| plan.exec_orders[*idx]);
    for &idx in &by_exec_order {
      let module = snapshot.module_table.modules[idx].as_normal()?;
      if let DeterminedSideEffects::Analyzed(false) = module.side_effects {
        let dependency_has_side_effects =
          module.import_records.iter().filter_map(|rec| rec.resolved_module).any(|dependency| {
            dependency != idx
              && link_output.module_table[dependency].side_effects().has_side_effects()
          });
        link_output.module_table.modules[idx].as_normal_mut()?.side_effects =
          DeterminedSideEffects::Analyzed(dependency_has_side_effects);
      }
    }

    for &idx in &plan.added {
      let targets = snapshot.module_table.modules[idx]
        .import_records()
        .iter()
        .filter_map(|rec| rec.resolved_module)
        .filter(|target| !added.contains(target))
        .collect::<Vec<_>>();
      let effects = |link_output: &crate::stages::link_stage::LinkStageOutput| {
        targets
          .iter()
          .map(|target| {
            let meta = &link_output.metas[*target];
            (meta.shimmed_missing_exports.len(), meta.included_commonjs_export_symbol.len())
          })
          .collect::<Vec<_>>()
      };
      let before = effects(link_output);
      check_or_none!(
        crate::stages::link_stage::bind_appended_module_imports(link_output, &self.options, idx),
        "added_binding"
      );
      // Binding must not change what its targets export.
      check_or_none!(effects(link_output) == before, "added_binding_effects");
    }
    Some(())
  }

  /// Computes one module's link data against the cached link output: a changed module's, whose
  /// cached link data is its previous scan's, or an appended module's, seeded above. Its scan
  /// may renumber its symbols; every other module's cached link data that names them is then
  /// renamed when the relink applies.
  #[expect(clippy::too_many_lines)]
  fn prepare_native_module(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    idx: ModuleIdx,
    ctx: &PrepareContext,
  ) -> Option<PreparedDirtyModule> {
    let is_added = ctx.added.contains(&idx);
    // An appended barrel's link data is a whole link's (`seed_added_modules`).
    check_or_none!(
      idx != cached.link_output.runtime.id()
        && !self.cache.native_cell_modules.contains(&idx)
        && (is_added || !self.cache.export_star_modules.contains(&idx))
        && !cached.generate_state.protected_modules.contains(&idx),
      "protected",
      snapshot.module_table.modules[idx].stable_id().to_string()
    );
    let old_n = cached.link_output.module_table.modules.get(idx)?.as_normal()?;
    let new_n = snapshot.module_table.modules.get(idx)?.as_normal()?;
    let chunk_idx = if is_added {
      *ctx.added_chunks.get(&idx)?
    } else {
      cached.generate_state.chunk_graph.module_to_chunk.get(idx).and_then(|c| *c)?
    };
    let old_meta = &cached.link_output.metas[idx];
    // An appended CommonJS module is seeded with the wrapper that linking gives it (#653).
    let is_commonjs = is_added && new_n.exports_kind.is_commonjs();
    check_or_none!(
      old_meta.is_included
        && (matches!(old_meta.wrap_kind(), WrapKind::None)
          || is_commonjs && matches!(old_meta.wrap_kind(), WrapKind::Cjs))
        && !old_meta.has_dynamic_exports
        && !old_meta.is_tla_or_contains_tla_dependency
        && old_meta.shimmed_missing_exports.is_empty()
        && old_meta.deferred_imports.is_empty()
        && old_meta.temporal_symbols.is_empty()
        && old_meta.temporal_constants.is_empty()
        && old_meta.star_exports_from_external_modules.is_empty()
        && old_meta.included_commonjs_export_symbol.is_empty(),
      "old_meta"
    );

    check_or_none!(
      old_n.id == new_n.id
        && old_n.stable_id == new_n.stable_id
        && old_n.repr_name == new_n.repr_name
        && old_n.module_type == new_n.module_type
        && matches!(
          new_n.module_type,
          ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx
        )
        && (is_commonjs || !new_n.exports_kind.is_commonjs())
        && (old_n.def_format as u8) == (new_n.def_format as u8)
        && old_n.originative_resolved_id.id == new_n.originative_resolved_id.id,
      "basic_id"
    );

    let post_link_exports_kind = if is_commonjs {
      ExportsKind::CommonJs
    } else if matches!(new_n.exports_kind, ExportsKind::Esm) {
      ExportsKind::Esm
    } else if matches!(new_n.exports_kind, ExportsKind::None) && !new_n.meta.has_lazy_export() {
      linked_exports_kind(
        snapshot,
        ctx.unreachable.as_ref().unwrap_or(&cached.link_output.unreachable_modules),
        idx,
      )
    } else {
      ExportsKind::None
    };
    check_or_none!(post_link_exports_kind == old_n.exports_kind, "exports_kind");

    let post_link_side_effects = match new_n.side_effects {
      DeterminedSideEffects::Analyzed(true)
      | DeterminedSideEffects::UserDefined(_)
      | DeterminedSideEffects::NoTreeshake => new_n.side_effects,
      DeterminedSideEffects::Analyzed(false) => {
        let dep_has_se =
          new_n.import_records.iter().filter_map(|rec| rec.resolved_module).any(|dep_idx| {
            dep_idx != idx
              && cached.link_output.module_table[dep_idx].side_effects().has_side_effects()
          });
        DeterminedSideEffects::Analyzed(dep_has_se)
      }
    };
    check_or_none!(
      std::mem::discriminant(&post_link_side_effects)
        == std::mem::discriminant(&old_n.side_effects)
        && (post_link_side_effects.has_side_effects() == old_n.side_effects.has_side_effects()
          || ctx.side_effects_changed.contains(&idx)),
      "side_effects"
    );
    check_or_none!(
      !matches!(post_link_side_effects, DeterminedSideEffects::UserDefined(false))
        || !new_n.named_exports.is_empty()
        || cached.link_output.user_defined_entry_modules.contains(&idx),
      "user_defined_side_effect_free"
    );

    let old_hashbang =
      old_n.ecma_view.hashbang_range.map(|r| &old_n.source[r.start as usize..r.end as usize]);
    let new_hashbang =
      new_n.ecma_view.hashbang_range.map(|r| &new_n.source[r.start as usize..r.end as usize]);
    check_or_none!(
      old_hashbang == new_hashbang
        && old_n
          .ecma_view
          .directive_range
          .iter()
          .map(|r| &old_n.source[r.start as usize..r.end as usize])
          .eq(
            new_n
              .ecma_view
              .directive_range
              .iter()
              .map(|r| &new_n.source[r.start as usize..r.end as usize]),
          ),
      "hashbang_directives"
    );

    let forbidden_ast_usage = if is_commonjs {
      EcmaModuleAstUsage::TopLevelAwait
    } else {
      EcmaModuleAstUsage::TopLevelAwait
        | EcmaModuleAstUsage::IsCjsReexport
        | EcmaModuleAstUsage::ModuleOrExports
    };
    check_or_none!(
      !old_n.ast_usage.intersects(forbidden_ast_usage)
        && !new_n.ast_usage.intersects(forbidden_ast_usage),
      "ast_usage"
    );
    let is_no_treeshake = matches!(post_link_side_effects, DeterminedSideEffects::NoTreeshake)
      && snapshot.stmt_infos[idx].iter().all(|s| !s.force_tree_shaking);
    let mut forbidden_meta = if is_no_treeshake {
      EcmaViewMeta::Eval | EcmaViewMeta::HasLazyExport | EcmaViewMeta::HasStarExport
    } else {
      EcmaViewMeta::Eval
        | EcmaViewMeta::HasLazyExport
        | EcmaViewMeta::HasStarExport
        | EcmaViewMeta::TopExportedSideEffectsFreeFunction
    };
    if is_added {
      forbidden_meta.remove(EcmaViewMeta::HasStarExport);
    }
    check_or_none!(
      !old_n.meta.intersects(forbidden_meta)
        && !new_n.meta.intersects(forbidden_meta)
        && old_n.meta.contains(EcmaViewMeta::SafelyTreeshakeCommonjs)
          == new_n.meta.contains(EcmaViewMeta::SafelyTreeshakeCommonjs),
      "meta"
    );

    // Appended modules may import a changed module anew.
    let same_importers = old_n.ecma_view.importers == new_n.ecma_view.importers
      && old_n.ecma_view.importers_idx == new_n.ecma_view.importers_idx;
    let importers_gained_appended = ctx.structural && {
      let old_importers: FxHashSet<ModuleIdx> =
        old_n.ecma_view.importers_idx.iter().copied().collect();
      let new_importers: FxHashSet<ModuleIdx> =
        new_n.ecma_view.importers_idx.iter().copied().collect();
      old_importers
        .iter()
        .all(|importer| new_importers.contains(importer) || ctx.retired.contains(importer))
        && new_importers
          .iter()
          .all(|importer| old_importers.contains(importer) || ctx.added.contains(importer))
    };
    check_or_none!(
      old_n.ecma_view.enum_member_value_map.is_empty()
        && new_n.ecma_view.enum_member_value_map.is_empty()
        && new_n.ecma_view.mutations.is_empty()
        && new_n.ecma_view.json_module_none_self_reference_included_symbol.is_none()
        && new_n.ecma_view.dummy_record_set.is_empty()
        && new_n.ecma_view.rolldown_file_url_references.is_empty()
        && old_n.ecma_view.new_url_references.is_empty()
        && new_n.ecma_view.new_url_references.is_empty()
        && (is_commonjs || new_n.ecma_view.cjs_reexport_import_record_ids.is_empty())
        && old_n.ecma_view.import_attribute_map.is_empty()
        && new_n.ecma_view.import_attribute_map.is_empty()
        && (same_importers || importers_gained_appended)
        && old_n.ecma_view.dynamic_importers == new_n.ecma_view.dynamic_importers,
      "ecma_view_maps"
    );

    let old_db = cached.link_output.symbol_db[idx].as_ref()?;
    let new_db = snapshot.symbol_ref_db[idx].as_ref()?;
    // An appended module's seeded link data has no scope tree; its scan has.
    let old_scope_db = if is_added { new_db } else { old_db };

    // The scan may renumber the module's symbols. Pair every symbol that other cached link
    // data names with its new counterpart.
    let mut renamed: FxHashMap<SymbolRef, SymbolRef> = FxHashMap::default();
    let mut renamed_targets: FxHashSet<SymbolRef> = FxHashSet::default();
    let mut pair = |old: SymbolRef, new: SymbolRef| -> bool {
      match renamed.get(&old) {
        Some(paired) => *paired == new,
        None => {
          renamed.insert(old, new);
          renamed_targets.insert(new)
        }
      }
    };
    check_or_none!(
      pair(old_n.namespace_object_ref, new_n.namespace_object_ref)
        && pair(old_n.default_export_ref, new_n.default_export_ref),
      "facade_symbols"
    );
    match (old_n.hmr_hot_ref, new_n.hmr_hot_ref) {
      (Some(old_hot), Some(new_hot)) => check_or_none!(pair(old_hot, new_hot), "hmr_hot_ref"),
      (None, None) => {}
      _ => reject_with!("hmr_hot_ref"),
    }

    // Records to appended modules, or to modules that the previous scan's records didn't import,
    // are new edges; records to retired modules, or to modules that the new scan's records don't
    // import, are dropped ones (#626). Every other record pairs with the previous scan's, in
    // order.
    let targets = |module: &rolldown_common::NormalModule| -> FxHashSet<ModuleIdx> {
      module.import_records.iter().filter_map(|rec| rec.resolved_module).collect()
    };
    let (old_targets, new_targets) = (targets(old_n), targets(new_n));
    let is_new_edge = |rec: &rolldown_common::ResolvedImportRecord| {
      rec
        .resolved_module
        .is_some_and(|t| !is_added && (ctx.added.contains(&t) || !old_targets.contains(&t)))
    };
    let is_dropped_edge = |rec: &rolldown_common::ResolvedImportRecord| {
      rec
        .resolved_module
        .is_some_and(|t| ctx.retired.contains(&t) || !is_added && !new_targets.contains(&t))
    };
    let old_kept: Vec<ImportRecordIdx> = old_n
      .import_records
      .iter_enumerated()
      .filter(|(_, rec)| !is_dropped_edge(rec))
      .map(|(rec_idx, _)| rec_idx)
      .collect();
    let new_kept: Vec<ImportRecordIdx> = new_n
      .import_records
      .iter_enumerated()
      .filter(|(_, rec)| !is_new_edge(rec))
      .map(|(rec_idx, _)| rec_idx)
      .collect();
    check_or_none!(old_kept.len() == new_kept.len(), "import_records_len");
    let edges_changed =
      old_kept.len() != old_n.import_records.len() || new_kept.len() != new_n.import_records.len();
    let mut edge_targets: Vec<ModuleIdx> = new_n
      .import_records
      .iter()
      .filter(|rec| is_new_edge(rec))
      .chain(old_n.import_records.iter().filter(|rec| is_dropped_edge(rec)))
      .filter_map(|rec| rec.resolved_module)
      .filter(|target| !ctx.added.contains(target) && !ctx.retired.contains(target))
      .collect();
    // A binding that a dropped record of a module that the update retires resolved into another
    // module in the output, as a barrel's `export *` resolves it, made the record import that
    // module's chunk, which it may stop importing as it may a dropped record's target (#705). What
    // it stops importing of such a module must be one of the module's own exports, which its
    // record exports whoever imports it, or its native namespace, which a record imports without
    // an export. The interop namespace through which it forwards a CommonJS module's binding
    // (#674), the namespace object that a module without a native namespace materializes, and a
    // name that linking shimmed for a missing export are exports of its record only while a
    // record imports them.
    let kept_export = |symbol: SymbolRef| {
      crate::utils::native_records::is_native_namespace(&self.options, &cached.link_output, symbol)
        || !is_shimmed_export(&cached.link_output, symbol)
          && cached
            .generate_state
            .chunk_graph
            .module_to_chunk
            .get(symbol.owner)
            .copied()
            .flatten()
            .is_some_and(|chunk_idx| entry_signature(cached, chunk_idx).contains(&symbol))
    };
    let dropped_import = |owner: ModuleIdx| {
      format!(
        "{} no longer imports {}",
        new_n.stable_id,
        cached.link_output.module_table.modules[owner].stable_id()
      )
    };
    if !is_added {
      for symbol in old_n.named_imports.iter().filter_map(|(symbol, import)| {
        old_n.import_records[import.record_idx]
          .resolved_module
          .is_some_and(|target| ctx.retired.contains(&target))
          .then_some(*symbol)
      }) {
        let resolved = cached.link_output.symbol_db.canonical_ref_resolving_namespace(symbol);
        let owner = resolved.owner;
        if owner == idx || ctx.added.contains(&owner) || ctx.retired.contains(&owner) {
          continue;
        }
        check_or_none!(kept_export(resolved), "dropped_export", dropped_import(owner));
        if !edge_targets.contains(&owner) {
          edge_targets.push(owner);
        }
      }
      // Linking shims a name that a record imports from an empty module whether or not the
      // module uses the import, so a dropped record's import of a shim drops it, also where no
      // statement that stays or leaves reads it.
      for (&symbol, import) in &old_n.named_imports {
        if !is_dropped_edge(&old_n.import_records[import.record_idx]) {
          continue;
        }
        let canonical = cached.link_output.symbol_db.canonical_ref_for(symbol);
        check_or_none!(
          ctx.retired.contains(&canonical.owner)
            || !is_shimmed_export(&cached.link_output, canonical),
          "dropped_export",
          dropped_import(canonical.owner)
        );
      }
    }
    // A module in the output that an edit starts or stops importing is a plain ES module, so the
    // edge changes no CommonJS interop or wrapper that its other importers share. One that it
    // starts importing is already included; `keeps_inclusion` proves that one that it stops
    // importing stays included (#626).
    for &target in &edge_targets {
      let starts = new_targets.contains(&target);
      let edge = || {
        let target = cached.link_output.module_table[target].stable_id();
        if starts {
          format!("{} imports {target}", new_n.stable_id)
        } else {
          format!("{} no longer imports {target}", new_n.stable_id)
        }
      };
      let meta = &cached.link_output.metas[target];
      check_or_none!(
        cached.link_output.module_table[target]
          .as_normal()
          .is_some_and(|module| !module.exports_kind.is_commonjs())
          && matches!(meta.wrap_kind(), WrapKind::None)
          && !meta.has_dynamic_exports
          && meta.star_exports_from_external_modules.is_empty(),
        "edge_target",
        edge()
      );
      check_or_none!(!starts || meta.is_included, "edge_target_excluded", edge());
    }
    let mut old_to_new_record: FxHashMap<ImportRecordIdx, ImportRecordIdx> = FxHashMap::default();
    for (&old_rec_idx, &new_rec_idx) in old_kept.iter().zip(new_kept.iter()) {
      old_to_new_record.insert(old_rec_idx, new_rec_idx);
    }
    for (old_rec_idx, old_rec) in old_n.import_records.iter_enumerated() {
      if old_to_new_record.contains_key(&old_rec_idx) {
        continue;
      }
      check_or_none!(
        old_rec.kind == ImportKind::Import
          && !old_rec.meta.contains(ImportRecordMeta::IsExportStar)
          && old_rec.dynamic_import_expr_info.is_none(),
        "dropped_edge"
      );
      // The module's CommonJS interop maps name the symbols of its records of CommonJS modules,
      // which the edit removes with the record, and the relink keeps the maps.
      if let Some(target) = old_rec
        .resolved_module
        .and_then(|target| cached.link_output.module_table[target].as_normal())
      {
        check_or_none!(
          !target.exports_kind.is_commonjs(),
          "dropped_commonjs_edge",
          format!("{} no longer imports {}", new_n.stable_id, target.stable_id)
        );
      }
    }
    for (new_rec_idx, new_rec) in new_n.import_records.iter_enumerated() {
      if !is_new_edge(new_rec) {
        continue;
      }
      let target = new_rec.resolved_module?;
      check_or_none!(
        new_rec.kind == ImportKind::Import
          && !new_rec.meta.contains(ImportRecordMeta::IsExportStar)
          && new_rec.dynamic_import_expr_info.is_none()
          && new_rec.module_request != "\0rolldown/computed-import",
        "new_edge"
      );
      // Linking includes the appended module only through an edge that evaluates it.
      check_or_none!(
        cached.link_output.module_table[target].side_effects().has_side_effects()
          || !crate::hmr::utils::unused_side_effect_free_import(
            new_n,
            new_rec_idx,
            &cached.link_output.module_table.modules,
            new_db.ast_scopes.scoping(),
          ),
        "new_edge_unused"
      );
    }

    for (&old_rec_idx, &new_rec_idx) in old_kept.iter().zip(new_kept.iter()) {
      let old_rec = &old_n.import_records[old_rec_idx];
      let new_rec = &new_n.import_records[new_rec_idx];
      check_or_none!(
        old_rec.module_request == new_rec.module_request
          && new_rec.module_request != "\0rolldown/computed-import"
          && old_rec.kind == new_rec.kind
          && (matches!(new_rec.kind, ImportKind::Import | ImportKind::HotAccept)
            || is_commonjs && new_rec.kind == ImportKind::Require)
          && old_rec.state.resolved_module == new_rec.state.resolved_module
          && old_rec.meta.bits() == new_rec.meta.bits()
          && (is_added || !new_rec.meta.contains(ImportRecordMeta::IsExportStar))
          && new_rec.dynamic_import_expr_info.is_none(),
        "import_rec"
      );
      check_or_none!(pair(old_rec.namespace_ref, new_rec.namespace_ref), "record_namespace");
      if let Some(target_idx) = new_rec.resolved_module {
        // A barrel that re-exports with `export *` is protected so that no relink changes what
        // it re-exports, and so is every module that it re-exports or imports. An importer that
        // keeps its record of one, and the names that it imports, keeps the bindings that the
        // cached link output resolved, as long as the update leaves the target as it is, which
        // the protection of updated modules checks (#618, #621). Native cells, and the modules
        // that chunk decisions depend on, stay refused.
        check_or_none!(
          !self.cache.native_cell_modules.contains(&target_idx)
            && !cached.generate_state.decision_dependencies.contains(&target_idx),
          "target_protected",
          format!(
            "{} imports {}",
            new_n.stable_id,
            cached.link_output.module_table[target_idx].stable_id()
          )
        );
        match &cached.link_output.module_table[target_idx] {
          Module::Normal(target_n) => {
            let target_meta = &cached.link_output.metas[target_n.idx];
            // A record can skip evaluating its target and import a native cell instead, under a
            // certificate that depends on how the importer uses the names. An edit can grant it
            // while keeping the record and the names, and the relink keeps the record's chunk
            // imports; a certificate already granted protects its importer. So with native cells,
            // an edited module's record that could gain one links the whole graph: a namespace
            // import, or a name that resolves to a cell, of a target that the certificate admits
            // (`native_cell_evaluation_dependencies`) (#621).
            if !is_added
              && !self.cache.native_cell_modules.is_empty()
              && cached.generate_state.module_has_native_ns.get(target_idx).copied().unwrap_or(true)
              && !target_n.import_records.is_empty()
              && !target_n.side_effects.has_side_effects()
              && !target_n.meta.has_eval()
              && !target_meta.is_tla_or_contains_tla_dependency
              && target_meta.resolved_exports.len()
                == target_meta.sorted_and_non_ambiguous_resolved_exports.len()
            {
              let cell_owned = |symbol: SymbolRef| {
                let owner = cached.link_output.symbol_db.canonical_ref_for(symbol).owner;
                self.cache.native_cell_modules.contains(&owner)
                  || cached.link_output.module_table[owner].as_normal().is_some_and(|owner| {
                    crate::utils::native_records::is_native_cell(owner.id.as_str())
                  })
              };
              check_or_none!(
                !new_n.named_imports.values().any(|import| {
                  import.record_idx == new_rec_idx
                    && match &import.imported {
                      Specifier::Star => true,
                      Specifier::Literal(name) => target_meta
                        .resolved_exports
                        .get(name.as_str())
                        .is_none_or(|export| cell_owned(export.symbol_ref)),
                    }
                }),
                "kept_cell_route",
                format!(
                  "{} imports {}",
                  new_n.stable_id,
                  cached.link_output.module_table[target_idx].stable_id()
                )
              );
            }
            // A wrapped ES module's importers call its initializer first, which the relink
            // references as linking the whole graph does (#621).
            if matches!(target_meta.wrap_kind(), WrapKind::Cjs) {
              check_or_none!(
                old_db.flags.get(&old_rec.namespace_ref.symbol).map(|f| f.bits())
                  == new_db.flags.get(&new_rec.namespace_ref.symbol).map(|f| f.bits()),
                "cjs_ns_ref"
              );
            }
          }
          Module::External(_) => {
            check_or_none!(
              matches!(self.options.format, rolldown_common::OutputFormat::Esm),
              "ext_format"
            );
          }
        }
      }
      if self.options.is_dev_mode_enabled() && !is_added {
        let old_unused = crate::hmr::utils::unused_side_effect_free_import(
          old_n,
          old_rec_idx,
          &cached.link_output.module_table.modules,
          old_db.ast_scopes.scoping(),
        );
        let new_unused = crate::hmr::utils::unused_side_effect_free_import(
          new_n,
          new_rec_idx,
          &cached.link_output.module_table.modules,
          new_db.ast_scopes.scoping(),
        );
        check_or_none!(old_unused == new_unused, "unused_se_free_import");
      }
    }

    // Named imports from appended modules bind to their exports; the others pair with the
    // previous scan's.
    let old_named = old_n
      .named_imports
      .iter()
      .filter(|(_, ni)| !is_dropped_edge(&old_n.import_records[ni.record_idx]))
      .collect::<Vec<_>>();
    let new_named = new_n
      .named_imports
      .iter()
      .filter(|(_, ni)| !is_new_edge(&new_n.import_records[ni.record_idx]))
      .collect::<Vec<_>>();
    check_or_none!(old_named.len() == new_named.len(), "named_imports_len");
    // Whether an import lets the exports object escape (`HasComputedMemberWrite`) changes what
    // linking inlines only for a target with a CommonJS constant, so an edit that starts or stops
    // passing on another module's binding, or calling its methods, links nothing else (#693).
    let import_flags = |db: &SymbolRefDbForModule,
                        symbol: SymbolRef,
                        record: &rolldown_common::ResolvedImportRecord| {
      let mut flags = db.flags.get(&symbol.symbol).copied().unwrap_or(SymbolRefFlags::empty());
      if !record
        .resolved_module
        .is_some_and(|target| escape_drops_constants(snapshot, target, true))
      {
        flags.remove(SymbolRefFlags::HasComputedMemberWrite);
      }
      flags.bits()
    };
    let mut kept_named_imports = Vec::with_capacity(old_named.len());
    for ((&old_sym, old_ni), (&new_sym, new_ni)) in old_named.into_iter().zip(new_named) {
      check_or_none!(
        old_ni.imported == new_ni.imported
          && old_ni.imported_as == old_sym
          && new_ni.imported_as == new_sym
          && old_to_new_record.get(&old_ni.record_idx) == Some(&new_ni.record_idx)
          && pair(old_sym, new_sym)
          && import_flags(old_db, old_sym, &old_n.import_records[old_ni.record_idx])
            == import_flags(new_db, new_sym, &new_n.import_records[new_ni.record_idx]),
        "named_import_item"
      );
      let canon = cached.link_output.symbol_db.canonical_ref_for(old_sym);
      check_or_none!(
        (is_no_treeshake
          || !canon.is_side_effect_free_function(
            &cached.link_output.symbol_db,
            &cached.link_output.module_table.modules,
          ))
          && !cached.link_output.global_constant_symbol_map.contains_key(&canon),
        "named_import_canon"
      );
      // Linking names the facade of an external module's default export after one of the
      // imports that it merges (`check_retired_modules`), which the relink keeps.
      if let Some(external) = old_n.import_records[old_ni.record_idx]
        .resolved_module
        .filter(|target| cached.link_output.module_table.modules[*target].is_external())
        && matches!(&old_ni.imported, Specifier::Literal(name) if name.as_str() == "default")
      {
        check_or_none!(
          old_sym.name(&cached.link_output.symbol_db) == new_sym.name(&snapshot.symbol_ref_db),
          "external_default_name",
          format!(
            "{} renames its default import of {}",
            new_n.stable_id,
            cached.link_output.module_table.modules[external].stable_id()
          )
        );
      }
      kept_named_imports.push((old_sym, new_sym));
    }
    let mut new_edge_links = Vec::new();
    let mut new_edge_aliases = Vec::new();
    for (&new_sym, new_ni) in &new_n.named_imports {
      let rec = &new_n.import_records[new_ni.record_idx];
      if !is_new_edge(rec) {
        continue;
      }
      let target = rec.resolved_module?;
      let target_n = cached.link_output.module_table[target].as_normal()?;
      // A new record imports an appended CommonJS module through the record's namespace, as
      // `match_import_with_export` binds an import of one (#653).
      if target_n.exports_kind.is_commonjs() {
        match &new_ni.imported {
          Specifier::Star => new_edge_links.push((new_sym, rec.namespace_ref)),
          Specifier::Literal(name) => new_edge_aliases.push((
            new_sym,
            NamespaceAlias { property_name: name.clone(), namespace_ref: rec.namespace_ref },
          )),
        }
        continue;
      }
      let bound = match &new_ni.imported {
        Specifier::Star => target_n.namespace_object_ref,
        Specifier::Literal(name) => {
          let export = cached.link_output.metas[target].resolved_exports.get(name.as_str())?;
          check_or_none!(
            !export.came_from_commonjs && export.potentially_ambiguous_symbol_refs.is_none(),
            "new_edge_export"
          );
          // A forward of a CommonJS module's binding binds the import to the forwarding record's
          // interop namespace, as `match_import_with_export` follows the forward (#674). Linking
          // would also mark the namespace for a JSX use of the import, which the relink doesn't.
          let canonical = cached.link_output.symbol_db.canonical_ref_for(export.symbol_ref);
          if let Some(alias) = &cached.link_output.symbol_db.get(canonical).namespace_alias {
            check_or_none!(
              !new_db.flags.get(&new_sym.symbol).is_some_and(|flags| {
                flags.intersects(
                  SymbolRefFlags::MustStartWithCapitalLetterForJSX
                    | SymbolRefFlags::UsedAsJSXMemberExprRoot,
                )
              }),
              "new_edge_forward_jsx",
              format!("{} imports {name} from {}", new_n.stable_id, target_n.stable_id)
            );
            new_edge_aliases.push((new_sym, alias.clone()));
            continue;
          }
          export.symbol_ref
        }
      };
      new_edge_links.push((new_sym, cached.link_output.symbol_db.canonical_ref_for(bound)));
    }
    check_or_none!(
      is_added
        || !writes_commonjs_exports(new_n, new_db, is_new_edge, |target, named| {
          escape_drops_constants(snapshot, target, named)
        }),
      "cjs_computed_write",
      new_n.stable_id.to_string()
    );

    check_or_none!(old_n.named_exports.len() == new_n.named_exports.len(), "named_exports_len");
    for (name, old_exp) in &old_n.named_exports {
      let Some(new_exp) = new_n.named_exports.get(name) else {
        reject_with!("named_exports");
      };
      check_or_none!(
        old_exp.came_from_commonjs == is_commonjs
          && new_exp.came_from_commonjs == is_commonjs
          && pair(old_exp.referenced, new_exp.referenced)
          // An appended module's exports of imported bindings are bound (`seed_added_modules`).
          && (is_added
            || !old_n.named_imports.contains_key(&old_exp.referenced)
              && !new_n.named_imports.contains_key(&new_exp.referenced))
          && old_scope_db.is_facade_symbol(old_exp.referenced.symbol)
            == new_db.is_facade_symbol(new_exp.referenced.symbol)
          && old_scope_db.symbol_name(old_exp.referenced.symbol)
            == new_db.symbol_name(new_exp.referenced.symbol)
          && old_db.flags.get(&old_exp.referenced.symbol).map(|f| f.bits())
            == new_db.flags.get(&new_exp.referenced.symbol).map(|f| f.bits()),
        "named_export_item"
      );
    }
    // Two old symbols mapped to one new one would merge bindings.
    check_or_none!(renamed.len() == renamed_targets.len(), "renamed_injective");
    let renumbered = renamed.iter().any(|(old, new)| old != new);
    if renumbered {
      self.check_renumbering(cached, snapshot, idx, ctx, &renamed)?;
    }
    let to_new = |symbol: SymbolRef| -> Option<SymbolRef> {
      if symbol.owner != idx {
        return Some(symbol);
      }
      match renamed.get(&symbol) {
        Some(new) => Some(*new),
        None if !renumbered => Some(symbol),
        None => None,
      }
    };

    let new_ast = snapshot.index_ecma_ast[idx].as_ref()?;
    let scoping = new_db.ast_scopes.scoping();
    let mut scope_lexical_bindings = Vec::new();
    for symbol in scoping.symbol_ids() {
      let flags = scoping.symbol_flags(symbol);
      if scoping.symbol_scope_id(symbol) == scoping.root_scope_id()
        && flags.intersects(
          oxc::syntax::symbol::SymbolFlags::BlockScopedVariable
            | oxc::syntax::symbol::SymbolFlags::Class,
        )
      {
        scope_lexical_bindings.push(symbol);
      }
    }
    let mut authored_lexical = FxHashSet::default();
    let mut top_level_vars = FxHashSet::default();
    let mut native_default_expression = false;
    for statement in &new_ast.program().body {
      if let Statement::ImportDeclaration(import) = statement
        && import.phase == Some(ImportPhase::Defer)
      {
        reject_with!("import_defer");
      }
      let variables = match statement {
        Statement::ExportDeclaration(export) => match &export.declaration {
          Declaration::VariableDeclaration(declaration) => Some(declaration),
          _ => None,
        },
        Statement::VariableDeclaration(declaration) => Some(declaration),
        _ => None,
      };
      if let Some(declaration) = variables {
        for declarator in &declaration.declarations {
          for binding in declarator.id.get_binding_identifiers() {
            top_level_vars.insert(binding.symbol_id());
            if declaration.kind.is_lexical() {
              authored_lexical.insert(binding.symbol_id());
            }
          }
        }
      }
      match statement {
        Statement::ExportDeclaration(export) => {
          if let Declaration::ClassDeclaration(class) = &export.declaration
            && let Some(id) = &class.id
          {
            authored_lexical.insert(id.symbol_id());
          }
        }
        Statement::ClassDeclaration(class) => {
          if let Some(id) = &class.id {
            authored_lexical.insert(id.symbol_id());
          }
        }
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
          ExportDefaultDeclarationKind::FunctionDeclaration(_) => {}
          ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            authored_lexical.insert(
              class.id.as_ref().map_or(new_n.default_export_ref.symbol, |id| id.symbol_id()),
            );
          }
          oxc::ast::ast::match_expression!(ExportDefaultDeclarationKind) => {
            authored_lexical.insert(new_n.default_export_ref.symbol);
            native_default_expression = true;
          }
          _ => {}
        },
        _ => {}
      }
    }
    // `preserve_temporal_bindings` skips CommonJS modules, whose constants `seed_added_modules`
    // gathers.
    check_or_none!(
      (is_added || native_default_expression == old_meta.native_default_expression)
        && scope_lexical_bindings.iter().all(|sym| authored_lexical.contains(sym))
        && (is_commonjs
          || new_n
            .constant_export_map
            .keys()
            .all(|sym| top_level_vars.contains(sym) || authored_lexical.contains(sym))
            && !cached.link_output.global_constant_symbol_map.keys().any(|k| k.owner == idx)),
      "temporal_or_const"
    );

    // Link data the cached link computed carries over to the renamed symbols.
    let mut new_classic_data = new_db.classic_data.clone();
    let mut new_flags = new_db.flags.clone();
    for (&old, &new) in &renamed {
      let mut data = old_db.classic_data[old.symbol].clone();
      if let Some(link) = data.link {
        data.link = Some(to_new(link)?);
      }
      if let Some(alias) = &mut data.namespace_alias {
        alias.namespace_ref = to_new(alias.namespace_ref)?;
      }
      new_classic_data[new.symbol] = data;
    }
    for (&old_rec_idx, &new_rec_idx) in old_kept.iter().zip(new_kept.iter()) {
      let old_ns = old_n.import_records[old_rec_idx].namespace_ref;
      let new_ns = new_n.import_records[new_rec_idx].namespace_ref;
      if renamed.get(&old_ns) == Some(&new_ns)
        && let Some(&flags) = old_db.flags.get(&old_ns.symbol)
      {
        new_flags.insert(new_ns.symbol, flags);
      }
    }
    for &(new_sym, target) in &new_edge_links {
      new_classic_data[new_sym.symbol].link = Some(target);
    }
    for (new_sym, alias) in new_edge_aliases {
      new_classic_data[new_sym.symbol].namespace_alias = Some(alias);
    }
    // The `require_*` wrapper that `seed_added_modules` declared after the scanned symbols and
    // statements, as `create_wrapper` declares it.
    let wrapper = if is_commonjs { old_meta.wrapper_ref } else { None };
    if let Some(wrapper) = wrapper {
      let symbol = new_classic_data.push(Default::default());
      debug_assert_eq!(symbol, wrapper.symbol);
      new_flags.insert(wrapper.symbol, SymbolRefFlags::IsFacade);
    }

    let mut new_stmt_infos = snapshot.stmt_infos[idx].clone();
    if let Some(wrapper) = wrapper {
      let stmt_info = crate::stages::link_stage::cjs_wrapper_stmt_info(
        wrapper,
        &cached.link_output.runtime,
        &self.options,
      );
      let stmt_info_idx = new_stmt_infos.add_stmt_info(stmt_info);
      debug_assert_eq!(Some(stmt_info_idx), old_meta.wrapper_stmt_info);
    }
    if matches!(post_link_exports_kind, ExportsKind::Esm) {
      let mut referenced_symbols = vec![];
      let mut declared_symbols: DeclaredSymbols = smallvec![];
      if !old_meta.is_canonical_exports_empty()
        || self.options.generated_code.symbols
        || old_meta.has_dynamic_exports
        || !old_meta.star_exports_from_external_modules.is_empty()
      {
        referenced_symbols.push(cached.link_output.runtime.resolve_symbol("__exportAll").into());
        for (_, export) in old_meta.canonical_exports(false) {
          referenced_symbols.push(to_new(export.symbol_ref)?.into());
        }
      }
      declared_symbols.push(TaggedSymbolRef::normal(new_n.namespace_object_ref));
      let namespace_stmt_info = StmtInfo {
        declared_symbols,
        referenced_symbols,
        eval_flags: false.into(),
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),
        ..Default::default()
      };
      new_stmt_infos.replace_namespace_stmt_info(namespace_stmt_info);
    }

    let mut depended_runtime_helper_map = DependedRuntimeHelperMap::default();
    let mut symbols_to_be_declared = Vec::new();
    let mut cjs_namespace_renames = Vec::new();
    let keep_names = self.options.keep_names;
    for (stmt_info_idx, stmt_info) in new_stmt_infos.infos.iter_mut_enumerated() {
      check_or_none!(
        !stmt_info
          .meta
          .intersects(StmtInfoMeta::HasDummyRecord | StmtInfoMeta::NonStaticDynamicImport),
        "stmt_meta"
      );
      for &rec_id in &stmt_info.import_records {
        let rec = &new_n.import_records[rec_id];
        let Some(module_idx) = rec.state.resolved_module else {
          continue;
        };
        if let Module::Normal(importee) = &cached.link_output.module_table[module_idx]
          && rec.kind == ImportKind::Import
        {
          let importee_linking_info = &cached.link_output.metas[importee.idx];
          if matches!(importee_linking_info.wrap_kind(), WrapKind::Cjs) {
            stmt_info.eval_flags = importee.side_effects.has_side_effects().into();
            stmt_info.referenced_symbols.push(importee_linking_info.wrapper_ref?.into());
            let needs_toesm =
              if let Some(info) = cached.link_output.safely_merge_cjs_ns_map.get(&importee.idx) {
                info.needs_interop
              } else {
                import_record_needs_interop(new_n, rec_id)
              };
            if needs_toesm {
              depended_runtime_helper_map.push(RuntimeHelper::ToEsm, stmt_info_idx);
            }
            symbols_to_be_declared.push((rec.namespace_ref, stmt_info_idx));
            cjs_namespace_renames
              .push((rec.namespace_ref.symbol, concat_string!("import_", importee.repr_name)));
          } else if matches!(importee_linking_info.wrap_kind(), WrapKind::Esm) {
            // `import ... from 'wrapped'` calls `init_wrapped()`, as the link stage's
            // `reference_needed_symbols` makes it.
            stmt_info.eval_flags = importee.side_effects.has_side_effects().into();
            stmt_info.referenced_symbols.push(importee_linking_info.wrapper_ref?.into());
          }
        } else if rec.kind == ImportKind::Require {
          // `require('cjs')` calls `require_cjs()`; `seed_added_modules` proved that the module
          // is wrapped.
          let wrapper_ref = cached.link_output.metas[module_idx].wrapper_ref?;
          stmt_info.referenced_symbols.push(wrapper_ref.into());
        }
      }
      if keep_names && stmt_info.meta.intersects(StmtInfoMeta::KeepNamesType) {
        depended_runtime_helper_map.push(RuntimeHelper::Name, stmt_info_idx);
      }
    }
    for (symbol_ref, stmt_idx) in symbols_to_be_declared {
      new_stmt_infos.declare_symbol_for_stmt(stmt_idx, TaggedSymbolRef::normal(symbol_ref));
    }

    // The module's CommonJS interop maps, keyed by its renamed symbols.
    let mut named_import_to_cjs_module: FxHashMap<SymbolRef, ModuleIdx> = old_meta
      .named_import_to_cjs_module
      .iter()
      .filter_map(|(symbol, module)| Some((to_new(*symbol)?, *module)))
      .collect();
    let mut import_record_ns_to_cjs_module: FxHashMap<SymbolRef, ModuleIdx> = old_meta
      .import_record_ns_to_cjs_module
      .iter()
      .filter_map(|(symbol, module)| Some((to_new(*symbol)?, *module)))
      .collect();
    // New records' imports of appended CommonJS modules, as `update_cjs_module_meta` records
    // them (#653).
    let cjs_edge = |rec: &rolldown_common::ResolvedImportRecord| {
      rec.resolved_module.filter(|target| {
        is_new_edge(rec)
          && cached.link_output.module_table[*target]
            .as_normal()
            .is_some_and(|target| target.exports_kind.is_commonjs())
      })
    };
    let mut cjs_edges_added = false;
    for (&symbol, import) in &new_n.named_imports {
      if let Some(target) = cjs_edge(&new_n.import_records[import.record_idx]) {
        named_import_to_cjs_module.insert(symbol, target);
        cjs_edges_added = true;
      }
    }
    for rec in &new_n.import_records {
      if let Some(target) = cjs_edge(rec) {
        import_record_ns_to_cjs_module.insert(rec.namespace_ref, target);
        cjs_edges_added = true;
      }
    }

    // Resolves through the module's new link data, then the cached link output's.
    let canonical_for = |symbol: SymbolRef| -> SymbolRef {
      let mut canonical = symbol;
      loop {
        let link = if canonical.owner == idx {
          new_classic_data[canonical.symbol].link
        } else {
          cached.link_output.symbol_db.get(canonical).link
        };
        match link {
          Some(next) => canonical = next,
          None => return canonical,
        }
      }
    };
    let namespace_alias_of = |symbol: SymbolRef| {
      if symbol.owner == idx {
        new_classic_data[symbol.symbol].namespace_alias.as_ref()
      } else {
        cached.link_output.symbol_db.get(symbol).namespace_alias.as_ref()
      }
    };
    let renamed_back: FxHashMap<SymbolRef, SymbolRef> =
      renamed.iter().map(|(old, new)| (*new, *old)).collect();
    // Re-export chains of the module's own named imports are keyed by their previous symbols.
    let chain_of = |symbol: SymbolRef| {
      let key = if symbol.owner == idx {
        match renamed_back.get(&symbol) {
          Some(old) => *old,
          None if !renumbered => symbol,
          None => return None,
        }
      } else {
        symbol
      };
      cached.link_output.normal_symbol_exports_chain_map.get(&key)
    };
    let mut new_resolved_member_expr_refs = MemberExprRefResolutionMap::default();
    let mut new_folded_namespace_in_checks = FxHashMap::default();
    for stmt_info in new_stmt_infos.iter() {
      for reference in &stmt_info.referenced_symbols {
        if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = reference {
          let mut canonical_ref = canonical_for(member_expr_ref.object_ref);
          let Some(mut canonical_ref_owner) =
            cached.link_output.module_table[canonical_ref.owner].as_normal()
          else {
            continue;
          };
          let namespace_of = |module: &rolldown_common::NormalModule| {
            if module.idx == idx { new_n.namespace_object_ref } else { module.namespace_object_ref }
          };
          if member_expr_ref.prop_and_span_list.is_empty() {
            if let Some(key) = new_n.ecma_view.namespace_in_checks.get(&member_expr_ref.node_id) {
              let owner_exports_kind = if canonical_ref_owner.idx == idx {
                post_link_exports_kind
              } else {
                canonical_ref_owner.exports_kind
              };
              let owner_meta = &cached.link_output.metas[canonical_ref_owner.idx];
              if namespace_of(canonical_ref_owner) == canonical_ref
                && !matches!(owner_exports_kind, ExportsKind::CommonJs)
                && !owner_meta.has_dynamic_exports
              {
                let json_hides_key = key.as_str() != "default"
                  && crate::utils::import_options::is_json_module(canonical_ref_owner)
                  && !crate::utils::import_options::json_namespace_has_properties(
                    canonical_ref_owner,
                    &cached.link_output.module_table.modules,
                    |m_idx| {
                      cached.link_output.unreachable_modules.reaches(m_idx)
                        && !ctx.retired.contains(&m_idx)
                    },
                  );
                let value = if json_hides_key {
                  Some(false)
                } else {
                  match owner_meta.sorted_and_non_ambiguous_resolved_exports.get(key) {
                    Some(&came_from_commonjs) => (!came_from_commonjs).then_some(true),
                    None => Some(false),
                  }
                };
                if let Some(value) = value {
                  new_folded_namespace_in_checks.insert(member_expr_ref.node_id, value);
                  new_resolved_member_expr_refs.insert(
                    member_expr_ref.node_id,
                    rolldown_common::MemberExprRefResolution {
                      resolved: None,
                      prop_and_related_span_list: vec![],
                      depended_refs: vec![],
                      target_commonjs_exported_symbol: None,
                      reference_id: member_expr_ref.reference_id,
                    },
                  );
                }
              }
            }
            continue;
          }
          if matches!(canonical_ref_owner.module_type, ModuleType::Json) {
            reject_with!("member_expr_json");
          }
          let mut is_namespace_ref = namespace_of(canonical_ref_owner) == canonical_ref;
          let mut cursor = 0;
          let mut depended_refs = Vec::new();
          while cursor < member_expr_ref.prop_and_span_list.len() && is_namespace_ref {
            // A module's own namespace resolves through its exports, which `renamed` maps.
            check_or_none!(canonical_ref_owner.idx != idx || !renumbered, "member_expr_self");
            let prop = &member_expr_ref.prop_and_span_list[cursor];
            let name = &prop.name;
            let meta = &cached.link_output.metas[canonical_ref_owner.idx];
            if meta.has_dynamic_exports || meta.star_export_record_by_name.contains_key(name) {
              reject_with!("member_expr_dynamic_exports");
            }
            let export_symbol = meta.resolved_exports.get(name).filter(|re| {
              !re.came_from_commonjs
                && (name == "default"
                  || !crate::utils::import_options::is_json_module(canonical_ref_owner))
            })?;
            if !meta.sorted_and_non_ambiguous_resolved_exports.contains_key(name) {
              reject_with!("member_expr_ambiguous_export");
            }
            depended_refs.push(export_symbol.symbol_ref);
            if let Some(chains) =
              cached.link_output.normal_symbol_exports_chain_map.get(&export_symbol.symbol_ref)
            {
              if chains.iter().any(|item| {
                cached.link_output.module_table[item.owner].side_effects().has_side_effects()
              }) {
                reject_with!("member_expr_chain_side_effects");
              }
              depended_refs.extend(chains.iter().copied());
            }
            canonical_ref = canonical_for(export_symbol.symbol_ref);
            let Some(normal_module) =
              cached.link_output.module_table[canonical_ref.owner].as_normal()
            else {
              cursor += 1;
              break;
            };
            if matches!(normal_module.module_type, ModuleType::Json) {
              reject_with!("member_expr_json");
            }
            canonical_ref_owner = normal_module;
            cursor += 1;
            is_namespace_ref = namespace_of(canonical_ref_owner) == canonical_ref;
          }

          let mut target_commonjs_exported_symbol = None;
          if cursor < member_expr_ref.prop_and_span_list.len() {
            let maybe_namespace = depended_refs
              .last()
              .copied()
              .unwrap_or_else(|| canonical_for(member_expr_ref.object_ref));
            let maybe_ns_alias = namespace_alias_of(maybe_namespace);
            let continue_resolve =
              maybe_ns_alias.is_none_or(|ns| ns.property_name.as_str() == "default");
            // As a whole link decides it (`resolve_member_expr_refs`, #760).
            let import_namespace = if maybe_namespace.owner == idx {
              import_record_ns_to_cjs_module.get(&maybe_namespace).copied()
            } else {
              cached.link_output.metas[maybe_namespace.owner]
                .import_record_ns_to_cjs_module
                .get(&maybe_namespace)
                .copied()
            };
            let cjs_module_idx = continue_resolve
              .then(|| {
                if maybe_namespace.owner == idx {
                  named_import_to_cjs_module.get(&maybe_namespace).copied().or(import_namespace)
                } else {
                  let owner_meta = &cached.link_output.metas[maybe_namespace.owner];
                  owner_meta
                    .named_import_to_cjs_module
                    .get(&maybe_namespace)
                    .copied()
                    .or(import_namespace)
                    .or_else(|| {
                      (owner_meta.has_dynamic_exports
                        && cached.link_output.module_table[maybe_namespace.owner]
                          .as_normal()
                          .is_some_and(|owner| namespace_of(owner) == maybe_namespace))
                      .then_some(maybe_namespace.owner)
                    })
                }
              })
              .flatten();
            let node_mode = |namespace: SymbolRef| {
              snapshot
                .module_table
                .modules
                .get(canonical_for(namespace).owner)
                .and_then(Module::as_normal)
                .is_some_and(
                  rolldown_common::NormalModule::should_consider_node_esm_spec_for_static_import,
                )
            };
            let is_default = member_expr_ref.prop_and_span_list[cursor].name == "default";
            let resolvable = match maybe_ns_alias {
              Some(alias) => !is_default && node_mode(alias.namespace_ref),
              None => !is_default || (import_namespace.is_some() && node_mode(maybe_namespace)),
            };
            if let Some(cjs_idx) = cjs_module_idx
              && resolvable
              && let Some(m) = cached.link_output.metas[cjs_idx]
                .resolved_exports
                .get(&member_expr_ref.prop_and_span_list[cursor].name)
                .filter(|re| re.came_from_commonjs)
            {
              check_or_none!(!member_expr_ref.is_write, "member_expr_cjs_write");
              let default_is_module_exports = is_default;
              if default_is_module_exports
                && let Some(next_prop) = member_expr_ref.prop_and_span_list.get(cursor + 1)
              {
                if let Some(property) = cached.link_output.metas[cjs_idx]
                  .resolved_exports
                  .get(&next_prop.name)
                  .filter(|re| re.came_from_commonjs)
                {
                  let is_next_default = next_prop.name == "default";
                  if !(is_next_default && maybe_ns_alias.is_none()) {
                    cursor += 1;
                    target_commonjs_exported_symbol = Some((property.symbol_ref, is_next_default));
                    depended_refs.push(property.symbol_ref);
                  }
                }
              } else if default_is_module_exports {
                // Leave target_commonjs_exported_symbol as None
              } else {
                target_commonjs_exported_symbol = Some((m.symbol_ref, is_default));
              }
              depended_refs.push(m.symbol_ref);
            }
          }

          if let Some((cjs_sym, _)) = target_commonjs_exported_symbol {
            // An appended CommonJS module's constants are a whole link's, which no importer
            // writes (`seed_added_modules`).
            check_or_none!(
              ctx.added.contains(&cjs_sym.owner)
                || !cached.link_output.global_constant_symbol_map.contains_key(&cjs_sym),
              "member_expr_cjs_global_const"
            );
          }

          if cursor > 0 {
            check_or_none!(
              is_no_treeshake
                || !canonical_ref.is_side_effect_free_function(
                  &cached.link_output.symbol_db,
                  &cached.link_output.module_table.modules,
                ),
              "member_expr_se_free_fn"
            );
            check_or_none!(
              !cached.link_output.global_constant_symbol_map.contains_key(&canonical_ref),
              "member_expr_global_const"
            );
            depended_refs.push(member_expr_ref.object_ref);
            if let Some(refs) = chain_of(member_expr_ref.object_ref) {
              depended_refs.extend(refs.iter().copied());
            }
          }

          if cursor > 0 || target_commonjs_exported_symbol.is_some() {
            new_resolved_member_expr_refs.insert(
              member_expr_ref.node_id,
              rolldown_common::MemberExprRefResolution {
                resolved: Some(canonical_ref),
                prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..].to_vec(),
                depended_refs,
                target_commonjs_exported_symbol,
                reference_id: member_expr_ref.reference_id,
              },
            );
          }
        }
      }
    }

    // A record imports its runtime helpers, and the runtime symbols that its statements name,
    // from the runtime record, whose exports the relink keeps: linking the whole graph would
    // export any other (#653).
    let runtime = &cached.link_output.runtime;
    let chunk_graph = &cached.generate_state.chunk_graph;
    let runtime_exports = chunk_graph
      .module_to_chunk
      .get(runtime.id())
      .copied()
      .flatten()
      .map(|chunk_idx| &chunk_graph.chunk_table[chunk_idx].exports_to_other_chunks);
    let runtime_exported =
      |symbol: SymbolRef| runtime_exports.is_some_and(|exports| exports.contains_key(&symbol));

    let mut stmt_info_included = IndexBitSet::new(new_stmt_infos.len());
    let mut local_used_symbols: FxHashSet<SymbolRef> = FxHashSet::default();
    let mut stmt_queue: Vec<StmtInfoIdx> = Vec::new();
    let mut sym_queue: Vec<SymbolRef> = Vec::new();

    if matches!(post_link_exports_kind, ExportsKind::Esm) {
      stmt_queue.push(StmtInfos::NAMESPACE_STMT_IDX);
    }
    // The records that import or require an appended CommonJS module call its wrapper, which
    // the relink proves once every module is prepared (`commonjs_uses`).
    if let Some(wrapper) = wrapper {
      sym_queue.push(wrapper);
    }
    for &(sym_ref, _) in &old_meta.referenced_symbols_by_entry_point_chunk {
      if sym_ref.owner == idx {
        let sym_ref = to_new(sym_ref)?;
        for &decl_stmt in new_stmt_infos.declared_stmts_by_symbol(&sym_ref) {
          stmt_queue.push(decl_stmt);
        }
        sym_queue.push(sym_ref);
      }
    }
    for (name, exp) in &new_n.named_exports {
      let old_referenced = old_n.named_exports.get(name)?.referenced;
      if ctx.is_used(cached, old_referenced) {
        for &decl_stmt in new_stmt_infos.declared_stmts_by_symbol(&exp.referenced) {
          stmt_queue.push(decl_stmt);
        }
        sym_queue.push(exp.referenced);
      }
    }

    let forced_no_treeshake = matches!(post_link_side_effects, DeterminedSideEffects::NoTreeshake);
    if self.options.treeshake.is_some() && !forced_no_treeshake {
      let safely_treeshake_cjs = new_n.meta.contains(EcmaViewMeta::SafelyTreeshakeCommonjs)
        && self.options.treeshake.commonjs();
      for (stmt_info_id, stmt_info) in new_stmt_infos.iter_enumerated_without_namespace_stmt() {
        let has_side_effects = if safely_treeshake_cjs {
          stmt_info.eval_flags.contains(rolldown_common::StmtEvalFlags::UnknownSideEffect)
        } else {
          stmt_info.eval_flags.has_side_effect_for_tree_shaking()
        };
        if has_side_effects {
          stmt_queue.push(stmt_info_id);
        }
      }
    } else {
      for (stmt_info_id, stmt_info) in new_stmt_infos.iter_enumerated_without_namespace_stmt() {
        if self.options.is_dev_mode_enabled()
          && stmt_info.import_records.len() == 1
          && crate::hmr::utils::unused_side_effect_free_import(
            new_n,
            stmt_info.import_records[0],
            &cached.link_output.module_table.modules,
            scoping,
          )
        {
          continue;
        }
        if stmt_info.force_tree_shaking {
          if stmt_info.eval_flags.has_side_effect_for_tree_shaking() {
            stmt_queue.push(stmt_info_id);
          }
        } else {
          stmt_queue.push(stmt_info_id);
        }
      }
    }

    let prop_write_false = matches!(
      self.options.treeshake.property_write_side_effects(),
      rolldown_common::PropertyWriteSideEffects::False
    );
    let cached_declarations_included = |sym_ref: SymbolRef| {
      cached.link_output.stmt_infos[sym_ref.owner].declared_stmts_by_symbol(&sym_ref).iter().all(
        |decl_stmt| cached.link_output.metas[sym_ref.owner].stmt_info_included.has_bit(*decl_stmt),
      )
    };
    while !stmt_queue.is_empty() || !sym_queue.is_empty() {
      while let Some(stmt_info_id) = stmt_queue.pop() {
        if !stmt_info_included.set_bit(stmt_info_id) {
          continue;
        }
        let stmt_info = new_stmt_infos.get(stmt_info_id);
        for reference_ref in &stmt_info.referenced_symbols {
          if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = reference_ref
            && let Some(member_expr_resolution) =
              member_expr_ref.resolution(&new_resolved_member_expr_refs)
          {
            if let Some(resolved_ref) = member_expr_resolution.resolved {
              for &sym_ref in &member_expr_resolution.depended_refs {
                if sym_ref.owner == idx {
                  for &decl_stmt in new_stmt_infos.declared_stmts_by_symbol(&sym_ref) {
                    stmt_queue.push(decl_stmt);
                  }
                } else if let Module::Normal(_) = &cached.link_output.module_table[sym_ref.owner] {
                  check_or_none!(cached_declarations_included(sym_ref), "depended_ref_excluded");
                }
              }
              if resolved_ref.owner == idx {
                for &decl_stmt in new_stmt_infos.declared_stmts_by_symbol(&resolved_ref) {
                  stmt_queue.push(decl_stmt);
                }
              }
              sym_queue.push(resolved_ref);
            }
          } else {
            let original_ref = *reference_ref.symbol_ref();
            if original_ref.owner == idx {
              for &decl_stmt in new_stmt_infos.declared_stmts_by_symbol(&original_ref) {
                stmt_queue.push(decl_stmt);
              }
            }
            if let Some(chains) = chain_of(original_ref) {
              for &sym_ref in chains {
                if sym_ref.owner == idx {
                  for &decl_stmt in new_stmt_infos.declared_stmts_by_symbol(&sym_ref) {
                    stmt_queue.push(decl_stmt);
                  }
                } else if let Module::Normal(_) = &cached.link_output.module_table[sym_ref.owner] {
                  check_or_none!(cached_declarations_included(sym_ref), "chain_excluded");
                }
              }
            }
            sym_queue.push(original_ref);
          }
        }
      }
      while let Some(sym_ref) = sym_queue.pop() {
        if sym_ref.owner == idx {
          if !local_used_symbols.insert(sym_ref) {
            continue;
          }
          let mut canonical_ref = canonical_for(sym_ref);
          if let Some(ns) = namespace_alias_of(canonical_ref) {
            canonical_ref = ns.namespace_ref;
          }
          if canonical_ref.owner == idx {
            local_used_symbols.insert(canonical_ref);
            for &decl_stmt in new_stmt_infos.declared_stmts_by_symbol(&canonical_ref) {
              stmt_queue.push(decl_stmt);
            }
          } else {
            // An appended module's record declares the namespace that `seed_added_modules`
            // includes, as an activated lazy proxy reads its target's (#651).
            let appended_namespace = ctx.added.contains(&canonical_ref.owner)
              && cached.link_output.module_table[canonical_ref.owner]
                .as_normal()
                .is_some_and(|owner| owner.namespace_object_ref == canonical_ref)
              && cached.link_output.metas[canonical_ref.owner].namespace_included;
            check_or_none!(
              appended_namespace || ctx.is_used(cached, canonical_ref),
              "canon_not_in_used_symbol_refs",
              format!(
                "{} uses {}",
                new_n.stable_id,
                cached.link_output.module_table[canonical_ref.owner].stable_id()
              )
            );
            check_or_none!(
              !cached.link_output.metas[canonical_ref.owner]
                .temporal_symbols
                .contains_key(&canonical_ref),
              "canon_in_temporal_symbols"
            );
          }
          if prop_write_false
            && let Some(stmt_ids) = new_stmt_infos.symbol_ref_to_referenced_stmt_idx().get(&sym_ref)
          {
            for &s_id in stmt_ids {
              stmt_queue.push(s_id);
            }
          }
        } else {
          check_or_none!(
            sym_ref.owner != runtime.id() || runtime_exported(sym_ref),
            "runtime_helper_unexported",
            new_n.stable_id.to_string()
          );
          check_or_none!(ctx.is_used(cached, sym_ref), "sym_not_in_used_symbol_refs");
          check_or_none!(
            !cached.link_output.metas[sym_ref.owner].temporal_symbols.contains_key(&sym_ref),
            "sym_in_temporal_symbols"
          );
        }
      }
    }

    let mut normalized_runtime_helper = RuntimeHelper::default();
    for (helper, stmt_info_idxs) in depended_runtime_helper_map.iter() {
      if stmt_info_idxs.is_empty() {
        continue;
      }
      let any_included = stmt_info_idxs.iter().any(|s_idx| stmt_info_included.has_bit(*s_idx));
      normalized_runtime_helper.set(helper, any_included);
    }
    check_or_none!(
      is_added
        || normalized_runtime_helper == old_meta.depended_runtime_helper
        // An edited module whose records change may gain helpers, such as an appended CommonJS
        // module's interop, which `apply_native_graph_change` imports anew (#653).
        || edges_changed && normalized_runtime_helper.contains(old_meta.depended_runtime_helper),
      "runtime_helper_changed"
    );
    check_or_none!(
      normalized_runtime_helper.iter().all(|helper| {
        runtime_exported(runtime.resolve_symbol(RUNTIME_HELPER_NAMES[helper.bit_index()]))
      }),
      "runtime_helper_unexported",
      new_n.stable_id.to_string()
    );

    if !is_added {
      for &(old_sym, new_sym) in &kept_named_imports {
        check_or_none!(
          local_used_symbols.contains(&new_sym) == ctx.is_used(cached, old_sym),
          "named_import_used_changed"
        );
      }
    }

    let resolve_sym = |symbol: SymbolRef| -> SymbolRef {
      let canonical = canonical_for(symbol);
      namespace_alias_of(canonical).map_or(canonical, |ns| ns.namespace_ref)
    };

    let record_cjs_bailout = |sym_ref: SymbolRef, is_new: bool, out: &mut FxHashSet<ModuleIdx>| {
      let canonical_ref = if is_new {
        canonical_for(sym_ref)
      } else {
        cached.link_output.symbol_db.canonical_ref_for(sym_ref)
      };
      let own_new = is_new && canonical_ref.owner == idx;
      let ns_to_cjs = |namespace: SymbolRef| {
        if is_new && namespace.owner == idx {
          import_record_ns_to_cjs_module.get(&namespace).copied()
        } else {
          cached.link_output.metas[namespace.owner]
            .import_record_ns_to_cjs_module
            .get(&namespace)
            .copied()
        }
      };
      if let Some(cjs_idx) = ns_to_cjs(canonical_ref) {
        out.insert(cjs_idx);
      }
      let namespace_object = if own_new {
        Some(new_n.namespace_object_ref)
      } else {
        cached.link_output.module_table[canonical_ref.owner].namespace_object_ref()
      };
      if namespace_object == Some(canonical_ref) {
        out.insert(canonical_ref.owner);
      }
      let ns_alias = if own_new {
        new_classic_data[canonical_ref.symbol].namespace_alias.as_ref()
      } else {
        cached.link_output.symbol_db.get(canonical_ref).namespace_alias.as_ref()
      };
      if let Some(ns) = ns_alias
        && ns.property_name.as_str() == "default"
        && let Some(cjs_idx) = ns_to_cjs(ns.namespace_ref)
      {
        out.insert(cjs_idx);
      }
    };

    // The bindings that new records import, which are imported anew wherever their canonical
    // symbols live.
    let new_edge_bindings: FxHashSet<SymbolRef> = new_n
      .named_imports
      .iter()
      .filter(|(_, import)| is_new_edge(&new_n.import_records[import.record_idx]))
      .map(|(symbol, _)| *symbol)
      .collect();
    let mut new_cross_depended = FxIndexSet::default();
    // What the module still imports of the modules that it starts or stops importing.
    let mut new_edge_references = FxHashSet::default();
    let mut new_cjs_bailouts = FxHashSet::default();
    for (s_idx, s_info) in new_stmt_infos.iter_enumerated() {
      if !stmt_info_included.has_bit(s_idx) {
        continue;
      }
      for ref_sym in &s_info.referenced_symbols {
        let bound_anew = new_edge_bindings.contains(&match ref_sym {
          SymbolOrMemberExprRef::Symbol(symbol) => *symbol,
          SymbolOrMemberExprRef::MemberExpr(member) => member.object_ref,
        });
        let sym = match ref_sym {
          SymbolOrMemberExprRef::Symbol(s) => {
            record_cjs_bailout(*s, true, &mut new_cjs_bailouts);
            *s
          }
          SymbolOrMemberExprRef::MemberExpr(m) => {
            if let Some(res) = m.resolution(&new_resolved_member_expr_refs) {
              if let Some(resolved_ref) = res.resolved
                && res.target_commonjs_exported_symbol.is_none()
              {
                record_cjs_bailout(resolved_ref, true, &mut new_cjs_bailouts);
              }
            } else {
              record_cjs_bailout(m.object_ref, true, &mut new_cjs_bailouts);
            }
            let Some(s) = m.represent_symbol_ref(&new_resolved_member_expr_refs) else {
              continue;
            };
            s
          }
        };
        let resolved = resolve_sym(sym);
        // Symbols of appended modules, and of the modules that new records import, are imported
        // anew; the generate stage derives those imports (#626). So is a symbol that a new record
        // binds through an appended barrel, in a module of the output that isn't the record's
        // target, which is then a plain ES module, as such a target is (#629).
        let anew_elsewhere = bound_anew
          && resolved.owner != idx
          && !ctx.added.contains(&resolved.owner)
          && !edge_targets.contains(&resolved.owner);
        if anew_elsewhere {
          let meta = &cached.link_output.metas[resolved.owner];
          check_or_none!(
            cached.link_output.module_table[resolved.owner]
              .as_normal()
              .is_some_and(|owner| !owner.exports_kind.is_commonjs())
              && matches!(meta.wrap_kind(), WrapKind::None)
              && !meta.has_dynamic_exports
              && meta.is_included,
            "new_edge_owner",
            format!(
              "{} imports {}",
              new_n.stable_id,
              cached.link_output.module_table[resolved.owner].stable_id()
            )
          );
        }
        if resolved.owner != idx
          && !anew_elsewhere
          && (is_added
            || !ctx.added.contains(&resolved.owner) && !edge_targets.contains(&resolved.owner))
        {
          new_cross_depended.insert(resolved);
        }
        if edge_targets.contains(&resolved.owner) {
          new_edge_references.insert(resolved);
        }
      }
    }

    // What the module includes of appended CommonJS modules, which linking marks used and the
    // relink adds to their used symbols once every module is prepared (#653): the wrappers that
    // its statements call, the exports that it imports by name (`follow_cjs_namespace_alias`),
    // and every export of one whose namespace it observes or that it requires
    // (`include_cjs_bailout_exports`).
    let mut commonjs_uses = Vec::new();
    // A module requiring a CommonJS module observes its namespace, unless it assigns it to
    // `module.exports` (`scan_import_records_for_cjs_bailout`). Only appended modules require.
    let requires_bail_out = is_added
      && (!new_n.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport)
        || new_n.ecma_view.cjs_reexport_import_record_ids.len() > 1);
    for (s_idx, s_info) in new_stmt_infos.iter_enumerated() {
      if !stmt_info_included.has_bit(s_idx) {
        continue;
      }
      for reference in &s_info.referenced_symbols {
        if let SymbolOrMemberExprRef::Symbol(symbol) = reference
          && ctx.added.contains(&symbol.owner)
          && cached.link_output.metas[symbol.owner].wrapper_ref == Some(*symbol)
        {
          commonjs_uses.push(*symbol);
        }
      }
      for &rec_idx in &s_info.import_records {
        let rec = &new_n.import_records[rec_idx];
        if let Some(target) = rec.resolved_module
          && requires_bail_out
          && rec.kind != ImportKind::Import
          && cached.link_output.module_table[target]
            .as_normal()
            .is_some_and(|target| target.exports_kind.is_commonjs())
        {
          new_cjs_bailouts.insert(target);
        }
      }
    }
    for &symbol in &local_used_symbols {
      if let Some(alias) = namespace_alias_of(canonical_for(symbol))
        && alias.property_name.as_str() != "default"
        && let Some(target) = import_record_ns_to_cjs_module.get(&alias.namespace_ref)
        && ctx.added.contains(target)
        && let Some(export) =
          cached.link_output.metas[*target].resolved_exports.get(&alias.property_name)
      {
        commonjs_uses.push(export.symbol_ref);
      }
    }
    new_cjs_bailouts.retain(|cjs_idx| {
      if !ctx.added.contains(cjs_idx) {
        return true;
      }
      let exports = cached.link_output.metas[*cjs_idx].resolved_exports.values();
      commonjs_uses
        .extend(exports.filter(|export| export.came_from_commonjs).map(|export| export.symbol_ref));
      false
    });

    if is_added {
      // A CommonJS module whose namespace an appended module observes must already keep every
      // export it assigns.
      check_or_none!(
        new_cjs_bailouts.iter().all(|cjs_idx| {
          cached.link_output.metas[*cjs_idx]
            .resolved_exports
            .values()
            .filter(|export| export.came_from_commonjs)
            .all(|export| ctx.is_used(cached, export.symbol_ref))
        }),
        "added_cjs_bailout"
      );
    } else {
      let mut old_cross_depended = FxIndexSet::default();
      let mut old_cjs_bailouts = FxHashSet::default();
      for (s_idx, s_info) in cached.link_output.stmt_infos[idx].iter_enumerated() {
        if !old_meta.stmt_info_included.has_bit(s_idx) {
          continue;
        }
        for ref_sym in &s_info.referenced_symbols {
          let sym = match ref_sym {
            SymbolOrMemberExprRef::Symbol(s) => {
              record_cjs_bailout(*s, false, &mut old_cjs_bailouts);
              *s
            }
            SymbolOrMemberExprRef::MemberExpr(m) => {
              if let Some(res) = m.resolution(&old_meta.resolved_member_expr_refs) {
                if let Some(resolved_ref) = res.resolved
                  && res.target_commonjs_exported_symbol.is_none()
                {
                  record_cjs_bailout(resolved_ref, false, &mut old_cjs_bailouts);
                }
              } else {
                record_cjs_bailout(m.object_ref, false, &mut old_cjs_bailouts);
              }
              let Some(s) = m.represent_symbol_ref(&old_meta.resolved_member_expr_refs) else {
                continue;
              };
              s
            }
          };
          let resolved = cached.link_output.symbol_db.canonical_ref_resolving_namespace(sym);
          // What the module stops importing of a module that it may stop importing, also through
          // a namespace that it read a member of, must be one that the module's record exports
          // whoever imports it, or the module's own native namespace, which no export carries
          // (#705).
          check_or_none!(
            !edge_targets.contains(&resolved.owner)
              || new_edge_references.contains(&resolved)
              || kept_export(resolved),
            "dropped_export",
            dropped_import(resolved.owner)
          );
          if resolved.owner != idx
            && !ctx.retired.contains(&resolved.owner)
            && !edge_targets.contains(&resolved.owner)
          {
            old_cross_depended.insert(resolved);
          }
        }
      }
      // A bailout includes the exports that a module assigns through CommonJS, so one of a
      // module without them, as an activated lazy proxy's of its target (#651), includes nothing,
      // and so does a new one of a module that already uses every export it assigns, as an
      // edited module's import of a forward of its default export observes (#674).
      let bails_out = |idx: &ModuleIdx| {
        cached.link_output.metas[*idx]
          .resolved_exports
          .values()
          .any(|export| export.came_from_commonjs)
      };
      let uses_every_export = |idx: &ModuleIdx| {
        cached.link_output.metas[*idx]
          .resolved_exports
          .values()
          .filter(|export| export.came_from_commonjs)
          .all(|export| ctx.is_used(cached, export.symbol_ref))
      };
      old_cjs_bailouts.retain(bails_out);
      new_cjs_bailouts.retain(|cjs_idx| {
        bails_out(cjs_idx) && (old_cjs_bailouts.contains(cjs_idx) || !uses_every_export(cjs_idx))
      });
      check_or_none!(
        old_cross_depended.as_slice() == new_cross_depended.as_slice()
          && old_cjs_bailouts == new_cjs_bailouts,
        "cross_depended_changed"
      );
    }

    let mut new_local_db = SymbolRefDbForModule::new(
      oxc::semantic::Scoping::default(),
      idx,
      oxc::semantic::ScopeId::new(0),
    );
    new_local_db.classic_data = new_classic_data;
    new_local_db.flags = new_flags;

    Some(PreparedDirtyModule {
      idx,
      chunk_idx,
      exec_order: ctx.exec_orders.as_ref().map_or(old_n.exec_order, |orders| orders[idx]),
      post_link_exports_kind,
      post_link_side_effects,
      new_stmt_infos,
      new_local_db,
      cjs_namespace_renames,
      stmt_info_included,
      local_used_symbols,
      new_resolved_member_expr_refs,
      new_folded_namespace_in_checks,
      added: is_added,
      depended_runtime_helper: normalized_runtime_helper,
      native_default_expression,
      renamed: if renumbered { renamed } else { FxHashMap::default() },
      edges_changed,
      edge_targets,
      cjs_interop: cjs_edges_added
        .then_some((named_import_to_cjs_module, import_record_ns_to_cjs_module)),
      wrapper: wrapper
        .map(|wrapper| (wrapper.symbol, crate::stages::link_stage::cjs_wrapper_name(new_n))),
      commonjs_uses,
    })
  }

  /// Checks that renaming a module's renumbered symbols reaches every cached reference to them:
  /// only its own tables and the named imports of direct importers, which re-export none of
  /// them, name its symbols.
  fn check_renumbering(
    &self,
    cached: &CachedNativeRelinkState,
    snapshot: &NormalizedScanStageOutput,
    idx: ModuleIdx,
    ctx: &PrepareContext,
    renamed: &FxHashMap<SymbolRef, SymbolRef>,
  ) -> Option<()> {
    check_or_none!(ctx.structural, "renumber_structural");
    let link_output = &cached.link_output;
    let old_n = link_output.module_table.modules[idx].as_normal()?;
    let new_n = snapshot.module_table.modules[idx].as_normal()?;
    // A module that only `import()` reaches, as a lazy proxy, which an activation renumbers
    // (#651). Its importers' records name none of its symbols in development, and its entry
    // chunk's references are renamed with its own tables.
    let dynamic_only = old_n.ecma_view.importers_idx.is_empty()
      && new_n.ecma_view.importers_idx.is_empty()
      && link_output
        .entries
        .get(&idx)
        .is_none_or(|entries| entries.iter().all(|entry| entry.kind.is_dynamic_import()));
    check_or_none!(
      dynamic_only
        || old_n.ecma_view.dynamic_importers.is_empty()
          && new_n.ecma_view.dynamic_importers.is_empty()
          && !link_output.entries.contains_key(&idx),
      "renumber_entry"
    );
    check_or_none!(!link_output.user_defined_entry_modules.contains(&idx), "renumber_user_entry");
    check_or_none!(
      link_output.metas[idx].referenced_symbols_by_entry_point_chunk.iter().all(|(symbol, _)| {
        dynamic_only && (symbol.owner != idx || renamed.contains_key(symbol))
      }),
      "renumber_entry_symbols"
    );
    for importer in importers_of(old_n, new_n, idx, &ctx.retired) {
      let module = snapshot.module_table.modules[importer].as_normal()?;
      for (symbol, named_import) in &module.named_imports {
        let rec = &module.import_records[named_import.record_idx];
        if rec.resolved_module != Some(idx) {
          continue;
        }
        check_or_none!(
          !rec.meta.contains(ImportRecordMeta::IsExportStar)
            && !module.named_exports.values().any(|export| export.referenced == *symbol),
          "renumber_reexport"
        );
      }
      check_or_none!(
        !module.import_records.iter().any(|rec| {
          rec.resolved_module == Some(idx) && rec.meta.contains(ImportRecordMeta::IsExportStar)
        }),
        "renumber_export_star"
      );
    }
    let chunk_graph = &cached.generate_state.chunk_graph;
    if let Some(chunk_idx) = chunk_graph.module_to_chunk[idx] {
      let owned_renamed = |symbol: &SymbolRef| symbol.owner != idx || renamed.contains_key(symbol);
      check_or_none!(
        chunk_graph.chunk_table[chunk_idx].exports_to_other_chunks.keys().all(owned_renamed),
        "renumber_exports"
      );
      check_or_none!(
        chunk_graph.finalized_cjs_ns_map_idx_vec[chunk_idx]
          .iter()
          .all(|(from, to)| owned_renamed(from) && owned_renamed(to)),
        "renumber_cjs_namespaces"
      );
    }
    Some(())
  }
}

/// Renames a renumbered module's symbols in the cached link data of its direct importers and in
/// its own cached tables. `check_renumbering` proved that nothing else names them. The interop
/// of an appended CommonJS module already names its importers' new symbols.
pub(super) fn apply_native_renames(
  cached: &mut CachedNativeRelinkState,
  snapshot: &NormalizedScanStageOutput,
  idx: ModuleIdx,
  renamed: &FxHashMap<SymbolRef, SymbolRef>,
  retired: &FxHashSet<ModuleIdx>,
  added: &FxHashSet<ModuleIdx>,
) {
  let rename = |symbol: SymbolRef| renamed.get(&symbol).copied().unwrap_or(symbol);
  let link_output = &mut cached.link_output;
  let chunk_graph = &mut cached.generate_state.chunk_graph;
  let (Some(old_n), Some(new_n)) = (
    snapshot.module_table.modules[idx].as_normal(),
    snapshot.module_table.modules[idx].as_normal(),
  ) else {
    return;
  };
  let importers = importers_of(old_n, new_n, idx, retired);
  for importer in importers {
    let Some(module) = link_output.module_table.modules[importer].as_normal() else {
      continue;
    };
    let imports_from_module = module
      .named_imports
      .iter()
      .filter(|(_, ni)| module.import_records[ni.record_idx].resolved_module == Some(idx))
      .map(|(symbol, _)| *symbol)
      .collect::<Vec<_>>();
    for symbol in imports_from_module {
      let mut current = symbol;
      while let Some(next) = link_output.symbol_db.get(current).link {
        if next.owner == idx {
          link_output.symbol_db.get_mut(current).link = Some(rename(next));
          break;
        }
        current = next;
      }
    }
    for resolution in link_output.metas[importer].resolved_member_expr_refs.values_mut() {
      if let Some(resolved) = &mut resolution.resolved {
        *resolved = rename(*resolved);
      }
      for depended in &mut resolution.depended_refs {
        *depended = rename(*depended);
      }
      if let Some((symbol, _)) = &mut resolution.target_commonjs_exported_symbol {
        *symbol = rename(*symbol);
      }
    }
    if let Some(chunk_idx) = chunk_graph.module_to_chunk.get(importer).and_then(|c| *c) {
      let chunk = &mut chunk_graph.chunk_table[chunk_idx];
      for items in chunk.imports_from_other_chunks.values_mut() {
        for item in items {
          item.import_ref = rename(item.import_ref);
        }
      }
      rename_keys(&mut chunk.canonical_names, idx, &rename);
    }
  }

  if let Some(chunk_idx) = chunk_graph.module_to_chunk[idx] {
    rename_keys(&mut chunk_graph.chunk_table[chunk_idx].exports_to_other_chunks, idx, &rename);
    let namespaces = std::mem::take(&mut chunk_graph.finalized_cjs_ns_map_idx_vec[chunk_idx]);
    chunk_graph.finalized_cjs_ns_map_idx_vec[chunk_idx] =
      namespaces.into_iter().map(|(from, to)| (rename(from), rename(to))).collect();
  }
  rename_keys(&mut link_output.normal_symbol_exports_chain_map, idx, &rename);
  for (module, info) in &mut link_output.safely_merge_cjs_ns_map {
    if added.contains(module) {
      continue;
    }
    for namespace in &mut info.namespace_refs {
      *namespace = rename(*namespace);
    }
  }
  let meta = &mut link_output.metas[idx];
  for export in meta.resolved_exports.values_mut() {
    export.symbol_ref = rename(export.symbol_ref);
  }
  for (symbol, _) in &mut meta.referenced_symbols_by_entry_point_chunk {
    *symbol = rename(*symbol);
  }
  rename_keys(&mut meta.named_import_to_cjs_module, idx, &rename);
  rename_keys(&mut meta.import_record_ns_to_cjs_module, idx, &rename);
  meta.native_reexport_by_canonical = std::sync::OnceLock::new();
  meta.reexports_commonjs_property_cache.store(0, std::sync::atomic::Ordering::Relaxed);
  let retained = renamed
    .iter()
    .filter(|(old, _)| link_output.retained_export_symbols.contains(old))
    .map(|(_, new)| *new)
    .collect::<Vec<_>>();
  for symbol in retained {
    link_output.retained_export_symbols.insert(symbol);
  }
}

/// Re-keys the entries of `map` that `idx` owns. All of them are removed before any is inserted,
/// as a renamed symbol may take another's previous number.
fn rename_keys<V>(
  map: &mut FxHashMap<SymbolRef, V>,
  idx: ModuleIdx,
  rename: &impl Fn(SymbolRef) -> SymbolRef,
) {
  let owned = map.keys().filter(|symbol| symbol.owner == idx).copied().collect::<Vec<_>>();
  let entries = owned
    .into_iter()
    .filter_map(|symbol| Some((rename(symbol), map.remove(&symbol)?)))
    .collect::<Vec<_>>();
  map.extend(entries);
}

/// The live modules that import `idx` before or after the update, other than itself.
fn importers_of(
  old_n: &rolldown_common::NormalModule,
  new_n: &rolldown_common::NormalModule,
  idx: ModuleIdx,
  retired: &FxHashSet<ModuleIdx>,
) -> Vec<ModuleIdx> {
  let mut importers = old_n
    .ecma_view
    .importers_idx
    .iter()
    .chain(new_n.ecma_view.importers_idx.iter())
    .copied()
    .filter(|importer| *importer != idx && !retired.contains(importer))
    .collect::<Vec<_>>();
  importers.sort_unstable();
  importers.dedup();
  importers
}

/// The cached modules that an import can resolve through into a CommonJS interop namespace that
/// `merge_cjs_namespace` merged into another of its record's (#683). A whole link derives every
/// record's imports before it merges the namespaces, so an import that resolves to one keeps the
/// namespace that its own record binds, while the cached symbols cross the merge.
#[derive(Default)]
struct MergedNamespaceReach {
  /// The modules whose records were merged and that re-export a binding through one of them, each
  /// mapped to itself, and the modules that re-export from one, through any chain of named,
  /// default, `export *` and `export * as` re-exports, each mapped to the module that it reaches.
  bindings: FxHashMap<ModuleIdx, ModuleIdx>,
  /// Those that reach a merged namespace itself, which a module re-exports as its namespace
  /// (`export * as`), so that any reference through it can resolve anew. A named or default
  /// re-export of a CommonJS binding keeps the namespace of its own record (`NamespaceAlias`).
  namespaces: FxHashMap<ModuleIdx, ModuleIdx>,
}

impl MergedNamespaceReach {
  fn of(cached: &CachedNativeRelinkState) -> Self {
    let modules = &cached.link_output.module_table.modules;
    let mut reach = Self::default();
    for merged in &cached.generate_state.chunk_graph.finalized_cjs_ns_map_idx_vec {
      for (namespace, kept) in merged {
        let holder = kept.owner;
        if namespace == kept || reach.namespaces.contains_key(&holder) {
          continue;
        }
        let Some(module) = modules[holder].as_normal() else {
          continue;
        };
        for export in module.named_exports.values() {
          if let Some(import) = module.named_imports.get(&export.referenced)
            && merged.get(&module.import_records[import.record_idx].namespace_ref) == Some(kept)
          {
            reach.bindings.insert(holder, holder);
            if matches!(import.imported, Specifier::Star) {
              reach.namespaces.insert(holder, holder);
            }
          }
        }
      }
    }
    if reach.bindings.is_empty() {
      return reach;
    }
    // Every cached module's importers, appended ones among them.
    let mut importers: FxHashMap<ModuleIdx, Vec<ModuleIdx>> = FxHashMap::default();
    for (idx, module) in modules.iter_enumerated() {
      for rec in module.as_normal().into_iter().flat_map(|module| &module.import_records) {
        if rec.kind == ImportKind::Import
          && let Some(target) = rec.resolved_module
        {
          importers.entry(target).or_default().push(idx);
        }
      }
    }
    let mut re_exported: FxHashMap<ModuleIdx, FxHashSet<ModuleIdx>> = FxHashMap::default();
    for map in [&mut reach.bindings, &mut reach.namespaces] {
      let mut queue = map.keys().copied().collect::<Vec<_>>();
      while let Some(target) = queue.pop() {
        let holder = map[&target];
        for &importer in importers.get(&target).into_iter().flatten() {
          if !map.contains_key(&importer)
            && re_exported
              .entry(importer)
              .or_insert_with(|| {
                modules[importer].as_normal().map(re_exported_modules).unwrap_or_default()
              })
              .contains(&target)
          {
            map.insert(importer, holder);
            queue.push(importer);
          }
        }
      }
    }
    reach
  }
}

/// The modules whose bindings `module` re-exports: through `export *`, or through an export of a
/// binding that one of its records imports from them, by name or as their namespace.
fn re_exported_modules(module: &rolldown_common::NormalModule) -> FxHashSet<ModuleIdx> {
  let exported = module
    .named_exports
    .values()
    .filter_map(|export| module.named_imports.get(&export.referenced))
    .map(|import| import.record_idx)
    .collect::<FxHashSet<_>>();
  module
    .import_records
    .iter_enumerated()
    .filter(|(rec_idx, rec)| {
      rec.kind == ImportKind::Import
        && (rec.meta.contains(ImportRecordMeta::IsExportStar) || exported.contains(rec_idx))
    })
    .filter_map(|(_, rec)| rec.resolved_module)
    .collect()
}

/// The module whose merged namespaces `reach` maps a target of one of `module`'s records to.
fn record_reach(
  module: &rolldown_common::NormalModule,
  reach: &FxHashMap<ModuleIdx, ModuleIdx>,
  record: impl Fn(ImportRecordIdx) -> bool,
) -> Option<ModuleIdx> {
  module.import_records.iter_enumerated().find_map(|(rec_idx, rec)| {
    let target =
      rec.resolved_module.filter(|_| rec.kind == ImportKind::Import && record(rec_idx))?;
    reach.get(&target).copied()
  })
}

/// Refuses an edit that can change how an import resolves into a module that `reach` maps
/// (#683): an edit of a module whose namespaces were merged, any edit of a module that imports
/// from one that reaches a merged namespace itself, through which its statements' references can
/// resolve anew, and an edit that adds or drops a record that imports from one that reaches a
/// binding of one.
fn check_merged_namespace_edits(
  cached: &CachedNativeRelinkState,
  snapshot: &NormalizedScanStageOutput,
  dirty_modules: &[ModuleIdx],
  reach: &MergedNamespaceReach,
) -> Option<()> {
  for &idx in dirty_modules {
    let (Some(old), Some(new)) = (
      cached.link_output.module_table.modules[idx].as_normal(),
      snapshot.module_table.modules[idx].as_normal(),
    ) else {
      continue;
    };
    let imports_from = |module: &rolldown_common::NormalModule| {
      let mut counts = FxHashMap::<ModuleIdx, usize>::default();
      for rec in &module.import_records {
        if let Some(target) = rec.resolved_module.filter(|_| rec.kind == ImportKind::Import) {
          *counts.entry(target).or_default() += 1;
        }
      }
      counts
    };
    let (old_imports, new_imports) = (imports_from(old), imports_from(new));
    let holder = reach
      .bindings
      .get(&idx)
      .copied()
      .filter(|holder| *holder == idx)
      .or_else(|| {
        [old, new].into_iter().find_map(|module| record_reach(module, &reach.namespaces, |_| true))
      })
      .or_else(|| {
        [old, new].into_iter().find_map(|module| {
          record_reach(module, &reach.bindings, |rec_idx| {
            module.import_records[rec_idx]
              .resolved_module
              .is_some_and(|target| old_imports.get(&target) != new_imports.get(&target))
          })
        })
      });
    if let Some(holder) = holder {
      reject_with!(
        "merged_namespace_import",
        cached.link_output.module_table.modules[holder].stable_id().to_string()
      );
    }
  }
  Some(())
}

/// Refuses an update that appends or retires a module that imports from a module that `reach`
/// maps (#683): its imports, and those of the modules that import from it, resolve anew.
fn check_merged_namespace_edges(
  cached: &CachedNativeRelinkState,
  snapshot: &NormalizedScanStageOutput,
  plan: &NativeGraphPlan,
  reach: &MergedNamespaceReach,
) -> Option<()> {
  let appended =
    plan.added.iter().filter_map(|idx| snapshot.module_table.modules[*idx].as_normal());
  let retired =
    plan.retired.iter().filter_map(|idx| cached.link_output.module_table.modules[*idx].as_normal());
  for module in appended.chain(retired) {
    if let Some(holder) = record_reach(module, &reach.bindings, |_| true) {
      reject_with!(
        "merged_namespace_import",
        cached.link_output.module_table.modules[holder].stable_id().to_string()
      );
    }
  }
  Some(())
}

/// Whether an import of `target` that lets a CommonJS module's exports object escape keeps
/// linking from inlining a constant (`resolve_member_expr_refs`), one that the scan of a CommonJS
/// module found among the exports that `target` resolves: its own, and, for a named import
/// (`named`), those that an `export *` of an ES module forwards (#693). A
/// `module.exports = require(...)` lets the required module's exports escape already.
fn escape_drops_constants(
  snapshot: &NormalizedScanStageOutput,
  target: ModuleIdx,
  named: bool,
) -> bool {
  let mut pending = vec![target];
  let mut seen = FxHashSet::default();
  while let Some(idx) = pending.pop() {
    if !seen.insert(idx) {
      continue;
    }
    let Some(module) = snapshot.module_table.modules.get(idx).and_then(Module::as_normal) else {
      continue;
    };
    if module.exports_kind.is_commonjs() {
      if module.constant_export_map.values().any(|constant| constant.commonjs_export) {
        return true;
      }
    } else if named {
      pending.extend(module.star_export_records().map(|(_, target)| target));
    }
  }
  false
}

/// Whether the cached module `idx` lets the exports object of a CommonJS module escape through an
/// import of a module that `stays`, marked `HasComputedMemberWrite`, which keeps linking from
/// inlining a constant of it (#693).
fn escapes_kept_commonjs_exports(
  link_output: &crate::stages::link_stage::LinkStageOutput,
  snapshot: &NormalizedScanStageOutput,
  idx: ModuleIdx,
  stays: impl Fn(ModuleIdx) -> bool,
) -> bool {
  let meta = &link_output.metas[idx];
  let named =
    meta.named_import_to_cjs_module.iter().map(|(&import, &target)| (import, target, true));
  let records =
    meta.import_record_ns_to_cjs_module.iter().map(|(&import, &target)| (import, target, false));
  named.chain(records).any(|(import, target, named)| {
    import
      .flags(&link_output.symbol_db)
      .is_some_and(|flags| flags.contains(SymbolRefFlags::HasComputedMemberWrite))
      && stays(target)
      && escape_drops_constants(snapshot, target, named)
  })
}

/// Whether `module` lets the exports object of a CommonJS module that one of `records` imports
/// escape through an import of it, marked `HasComputedMemberWrite`, after which
/// `resolve_member_expr_refs` inlines none of the module's constants: `drops_constants` tells
/// whether that changes what linking inlines for the target, through a named import or not.
fn writes_commonjs_exports(
  module: &rolldown_common::NormalModule,
  db: &SymbolRefDbForModule,
  records: impl Fn(&rolldown_common::ResolvedImportRecord) -> bool,
  drops_constants: impl Fn(ModuleIdx, bool) -> bool,
) -> bool {
  let escapes = |symbol: SymbolRef| {
    db.flags
      .get(&symbol.symbol)
      .is_some_and(|flags| flags.contains(SymbolRefFlags::HasComputedMemberWrite))
  };
  let drops = |rec: &rolldown_common::ResolvedImportRecord, named: bool| {
    records(rec) && rec.resolved_module.is_some_and(|target| drops_constants(target, named))
  };
  module.named_imports.iter().any(|(&symbol, import)| {
    escapes(symbol) && drops(&module.import_records[import.record_idx], true)
  }) || module.import_records.iter().any(|rec| escapes(rec.namespace_ref) && drops(rec, false))
}

/// The exports kind that linking gives `target`, which its scan found without exports: the first
/// record of a module that the graph reaches that imports it, in discovery order from the entries,
/// makes it an ES module, and the first that requires it a CommonJS one
/// (`determine_module_exports_kind`, #681).
fn linked_exports_kind(
  snapshot: &NormalizedScanStageOutput,
  unreachable: &UnreachableModules,
  target: ModuleIdx,
) -> ExportsKind {
  let modules = &snapshot.module_table.modules;
  let Some(module) = modules[target].as_normal() else {
    return ExportsKind::None;
  };
  let mut importers = module
    .ecma_view
    .importers_idx
    .iter()
    .copied()
    .filter(|&importer| unreachable.reaches(importer))
    .collect::<Vec<_>>();
  importers.sort_by_key(|&importer| (unreachable.discovery_order(importer), importer.index()));
  importers
    .into_iter()
    .filter_map(|importer| modules[importer].as_normal())
    .flat_map(|importer| importer.import_records.iter())
    .filter(|rec| rec.resolved_module == Some(target))
    .find_map(|rec| match rec.kind {
      ImportKind::Import if !module.meta.has_lazy_export() => Some(ExportsKind::Esm),
      ImportKind::Require => Some(ExportsKind::CommonJs),
      _ => None,
    })
    .unwrap_or(ExportsKind::None)
}

/// The bindings that a preserved module's record exports whatever imports it: its complete
/// declared interface. The runtime's helpers are exported on demand only.
fn entry_signature(cached: &CachedNativeRelinkState, chunk_idx: ChunkIdx) -> FxHashSet<SymbolRef> {
  let chunk = &cached.generate_state.chunk_graph.chunk_table[chunk_idx];
  let Some(module) = chunk.entry_module_idx() else {
    return FxHashSet::default();
  };
  if module == cached.link_output.runtime.id() {
    return FxHashSet::default();
  }
  cached.link_output.metas[module]
    .canonical_exports(false)
    .map(|(_, export)| export.symbol_ref)
    .collect()
}

/// Whether `symbol` is a name that linking shimmed in its owner's record for an import that the
/// owner doesn't export (`shim_missing_export`), which the record exports only while a record
/// imports it, though `entry_signature` counts it.
fn is_shimmed_export(
  link_output: &crate::stages::link_stage::LinkStageOutput,
  symbol: SymbolRef,
) -> bool {
  link_output
    .metas
    .get(symbol.owner)
    .is_some_and(|meta| meta.shimmed_missing_exports.values().any(|&shim| shim == symbol))
}
