use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  ConstExportMeta, EcmaModuleAstUsage, EcmaViewMeta, ExportsKind, ImportKind, ImportRecordMeta,
  IndexModules, MemberExprRef, Module, ModuleIdx, ModuleNamespaceIncludedReason, ModuleType,
  NormalModule, NormalizedBundlerOptions, RUNTIME_MODULE_ID, RuntimeHelper, StmtEvalFlags,
  StmtInfo, StmtInfoIdx, StmtInfoMeta, StmtInfos, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
  UsedExternalSymbols, UsedSymbolRefsBuilder, WrapKind, side_effects::DeterminedSideEffects,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{
  IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};

use rolldown_utils::IndexBitSet;
use rolldown_utils::indexmap::FxIndexMap;
use rolldown_utils::pass::Sealed;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  stages::link_stage::{
    LinkStage,
    passes::{EntryExportRoots, StatementRuntimeRequirements, UnreachableDynamicImports},
  },
  type_alias::IndexStmtInfos,
  types::linking_metadata::LinkingMetadataVec,
};

use super::{
  on_demand::{
    compute_body_demand_keys, is_gated_side_effect_stmt, side_effects_included_on_demand,
  },
  passes::{
    collect_depended_runtime_helpers, include_cjs_bailout_exports, include_runtime_symbol,
    preserve_reexported_interfaces,
  },
};

pub type StmtInclusionVec = IndexVec<ModuleIdx, IndexBitSet<StmtInfoIdx>>;
pub type ModuleInclusionVec = IndexBitSet<ModuleIdx>;
pub type ModuleNamespaceReasonVec = IndexVec<ModuleIdx, ModuleNamespaceIncludedReason>;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SymbolIncludeReason: u8 {
        const Normal = 1;
        const EntryExport = 1 << 1;
        /// See `has_dynamic_exports` in [`crate::types::linking_metadata::LinkingMetadata`]
        /// See the normal-import and normal-dynamic-import branches in
        /// `passes/reference_needed_symbols.rs`.
        const ReExportDynamicExports = 1 << 2;
        /// After transforming a JSON module to an ESM module, a default export is created that
        /// references all top-level properties of the JSON object. This flag tracks whether a
        /// property is being referenced by the export default object itself (self-reference).
        /// If a top-level property is only referenced by the export default object and not by
        /// any outer modules, it can be safely inlined in the final output.
        const JsonDefaultExportSelfReference = 1 << 3;
        /// Indicates that a symbol is included because it is used by a simulated facade chunk.
        /// Currently only used to track namespace symbol inclusion.
        /// https://github.com/rolldown/rolldown/blob/d6d65f9080e427cd9feef56eb7a110fbcf6c1414/crates/rolldown/src/stages/generate_stage/chunk_optimizer.rs#L422
        const SimulatedFacadeChunk = 1 << 4;
    }
}

pub struct IncludeContext<'a> {
  pub modules: &'a IndexModules,
  /// Per-module statement-info table, detached from `EcmaView` and held on
  /// `LinkStage` for the duration of the link/generate stages.
  pub stmt_infos: &'a IndexStmtInfos,
  pub symbols: &'a SymbolRefDb,
  pub is_included_vec: &'a mut StmtInclusionVec,
  pub is_module_included_vec: &'a mut ModuleInclusionVec,
  pub tree_shaking: bool,
  pub inline_const_smart: bool,
  pub runtime_idx: ModuleIdx,
  pub metas: &'a LinkingMetadataVec,
  pub used_symbol_refs: &'a mut UsedSymbolRefsBuilder,
  pub used_external_symbols: &'a mut UsedExternalSymbols,
  pub constant_symbol_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
  pub options: &'a NormalizedBundlerOptions,
  pub normal_symbol_exports_chain_map: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
  /// It is necessary since we can't mutate `module.meta` during the tree shaking process.
  /// see [rolldown_common::ecmascript::ecma_view::EcmaViewMeta]
  pub bailout_cjs_tree_shaking_modules: FxHashSet<ModuleIdx>,
  /// Tracks whether any new module was included during the current convergence iteration.
  /// Used to detect fixpoint without O(N) scanning of `is_module_included_vec`.
  pub module_inclusion_changed: bool,
  pub module_namespace_included_reason: &'a mut ModuleNamespaceReasonVec,
  pub json_module_none_self_reference_included_symbol: FxHashMap<ModuleIdx, FxHashSet<SymbolRef>>,
  /// User-defined entry modules (static and emitted, NOT dynamic). Exempt from
  /// on-demand side-effect inclusion: they are the requested program. Dynamic
  /// entries join through namespace/own-export body demand instead.
  pub entry_module_idxs: &'a FxHashSet<ModuleIdx>,
  /// Body-demand key (a module's own export or namespace object) -> the module whose gated
  /// side-effect statements using that key demands (see
  /// [`side_effects_included_on_demand`]). Consulted by [`include_symbol`] as symbols become
  /// used.
  pub body_demand_keys: &'a FxHashMap<SymbolRef, ModuleIdx>,
  /// The second module-inclusion bit: modules whose *gated* side-effect statements have joined
  /// because their body was demanded. Distinct from `is_module_included_vec` (structural
  /// inclusion), which deliberately skips those statements for
  /// [`side_effects_included_on_demand`] modules.
  pub body_demand_swept: FxHashSet<ModuleIdx>,
  /// Work queue of the inclusion engine (see [`drain_work_items`]). Edge producers push typed
  /// work items here instead of recursing; the public `include_*` entry points drain it to
  /// empty before returning. Invariant (debug-asserted at every public entry point): empty
  /// whenever control is outside the engine. Visible outside this module only because
  /// `chunk_optimizer` constructs `IncludeContext` with a struct literal; nothing outside this
  /// module should touch it.
  pub(in crate::stages) pending: Vec<WorkItem>,
}

impl<'a> IncludeContext<'a> {
  #[expect(clippy::too_many_arguments)]
  pub fn new(
    modules: &'a IndexModules,
    stmt_infos: &'a IndexStmtInfos,
    symbols: &'a SymbolRefDb,
    is_included_vec: &'a mut StmtInclusionVec,
    is_module_included_vec: &'a mut ModuleInclusionVec,
    runtime_idx: ModuleIdx,
    metas: &'a LinkingMetadataVec,
    used_symbol_refs: &'a mut UsedSymbolRefsBuilder,
    used_external_symbols: &'a mut UsedExternalSymbols,
    constant_symbol_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
    options: &'a NormalizedBundlerOptions,
    normal_symbol_exports_chain_map: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
    module_namespace_included_reason: &'a mut ModuleNamespaceReasonVec,
    entry_module_idxs: &'a FxHashSet<ModuleIdx>,
    body_demand_keys: &'a FxHashMap<SymbolRef, ModuleIdx>,
  ) -> Self {
    Self {
      modules,
      stmt_infos,
      symbols,
      is_included_vec,
      is_module_included_vec,
      tree_shaking: options.treeshake.is_some(),
      inline_const_smart: options.optimization.is_inline_const_smart_mode(),
      runtime_idx,
      metas,
      used_symbol_refs,
      used_external_symbols,
      constant_symbol_map,
      options,
      normal_symbol_exports_chain_map,
      bailout_cjs_tree_shaking_modules: FxHashSet::default(),
      module_inclusion_changed: false,
      module_namespace_included_reason,
      json_module_none_self_reference_included_symbol: FxHashMap::default(),
      entry_module_idxs,
      body_demand_keys,
      body_demand_swept: FxHashSet::default(),
      pending: Vec::new(),
    }
  }
}

/// A unit of inclusion work. Edge producers push these instead of recursing, which keeps the
/// engine iterative (no stack-depth limit on deep graphs) and makes the traversal a visible
/// queue rather than an implicit call stack. Visible outside this module only as the `pending`
/// field's type; not part of the module's API.
#[derive(Debug, Clone, Copy)]
pub(in crate::stages) enum WorkItem {
  /// Structurally include a module: sweep its side-effect statements and evaluate its
  /// side-effectful dependencies.
  Module(ModuleIdx),
  /// A symbol became used: retain its declaration, wrapper, and owner module.
  Symbol(SymbolRef, SymbolIncludeReason),
  /// Include one statement and follow everything it references.
  Statement(ModuleIdx, StmtInfoIdx),
}

/// Drain the work queue to empty. LIFO order mirrors the depth-first shape of the recursion this
/// engine replaced; the final inclusion sets are a monotone closure over the pushed edges, so
/// drain order affects only traversal order, never the result.
fn drain_work_items(ctx: &mut IncludeContext) {
  while let Some(item) = ctx.pending.pop() {
    match item {
      WorkItem::Module(module_idx) => handle_include_module(ctx, module_idx),
      WorkItem::Symbol(symbol_ref, reason) => handle_include_symbol(ctx, symbol_ref, reason),
      WorkItem::Statement(module_idx, stmt_info_idx) => {
        handle_include_statement(ctx, module_idx, stmt_info_idx);
      }
    }
  }
}

/// Include a symbol and check for CJS tree-shaking bailout.
///
/// Use this at most call sites. Only use bare [`include_symbol`] when you
/// explicitly want to skip the bailout check (e.g., for partial CJS member-
/// expression access or runtime symbols).
pub(super) fn include_symbol_and_check_cjs_bailout(
  ctx: &mut IncludeContext,
  symbol_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  push_symbol_and_check_cjs_bailout(ctx, symbol_ref, include_reason);
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

/// Engine-internal variant of [`include_symbol_and_check_cjs_bailout`]: enqueues the symbol and
/// runs the (order-independent) bailout check immediately, without draining. Handlers and edge
/// producers must use this — only the public entry points drain.
fn push_symbol_and_check_cjs_bailout(
  ctx: &mut IncludeContext,
  symbol_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) {
  ctx.pending.push(WorkItem::Symbol(symbol_ref, include_reason));
  check_cjs_bailout(ctx, symbol_ref);
}

/// Check if including this symbol should trigger CJS tree-shaking bailout.
/// This is called at `include_symbol` call sites where the symbol is NOT accessed
/// via a resolved member expression on a CJS namespace (i.e., where the full namespace
/// might be used opaquely). When we know only a specific property is accessed
/// (member expression with `target_commonjs_exported_symbol`), we skip this check
/// to allow CJS tree-shaking.
fn check_cjs_bailout(ctx: &mut IncludeContext, symbol_ref: SymbolRef) {
  let canonical_ref = ctx.symbols.canonical_ref_for(symbol_ref);

  // If the symbol is a CJS namespace import ref, bail out the target CJS module.
  if let Some(idx) =
    ctx.metas[canonical_ref.owner].import_record_ns_to_cjs_module.get(&canonical_ref)
  {
    ctx.bailout_cjs_tree_shaking_modules.insert(*idx);
  }
  // If the symbol IS a CJS module's namespace object, bail out that module.
  if ctx.modules[canonical_ref.owner].namespace_object_ref() == Some(canonical_ref) {
    ctx.bailout_cjs_tree_shaking_modules.insert(canonical_ref.owner);
  }

  // If the symbol has a namespace_alias importing "default" from a CJS module,
  // bail out that module (default import is the whole module.exports).
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    if let Some(idx) = ctx.metas[namespace_alias.namespace_ref.owner]
      .import_record_ns_to_cjs_module
      .get(&namespace_alias.namespace_ref)
    {
      if namespace_alias.property_name.as_str() == "default" {
        ctx.bailout_cjs_tree_shaking_modules.insert(*idx);
      }
    }
  }
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(in crate::stages::link_stage) fn include_statements(
    &mut self,
    unreachable_import_expression_node_ids: &UnreachableDynamicImports,
    statement_runtime_requirements: &Sealed<StatementRuntimeRequirements>,
    entry_export_roots: &EntryExportRoots,
  ) {
    let mut is_stmt_info_included_vec: StmtInclusionVec = self
      .module_table
      .modules
      .iter()
      .zip(self.stmt_infos.iter())
      .map(|(m, stmt_infos)| {
        m.as_normal().map_or(IndexBitSet::default(), |_| IndexBitSet::new(stmt_infos.len()))
      })
      .collect::<IndexVec<ModuleIdx, _>>();
    let mut used_symbol_refs = UsedSymbolRefsBuilder::default();
    let mut used_external_symbols = UsedExternalSymbols::default();
    let mut is_module_included_vec: ModuleInclusionVec =
      IndexBitSet::new(self.module_table.modules.len());
    let mut module_namespace_included_reason: ModuleNamespaceReasonVec =
      oxc_index::index_vec![ModuleNamespaceIncludedReason::empty(); self.module_table.len()];
    self.has_enum_inlining = self
      .module_table
      .modules
      .iter()
      .any(|m| m.as_normal().is_some_and(|n| !n.ecma_view.enum_member_value_map.is_empty()));
    let entry_module_idxs = self.user_defined_entry_module_idxs();
    let body_demand_keys = compute_body_demand_keys(
      &self.module_table.modules,
      &self.stmt_infos,
      &self.symbols,
      self.options.treeshake.is_some(),
      &entry_module_idxs,
    );
    let context = &mut IncludeContext::new(
      &self.module_table.modules,
      &self.stmt_infos,
      &self.symbols,
      &mut is_stmt_info_included_vec,
      &mut is_module_included_vec,
      self.runtime.id(),
      &self.metas,
      &mut used_symbol_refs,
      &mut used_external_symbols,
      &self.global_constant_symbol_map,
      self.options,
      &self.normal_symbol_exports_chain_map,
      &mut module_namespace_included_reason,
      &entry_module_idxs,
      &body_demand_keys,
    );

    let (user_defined_entries, mut dynamic_entries): (Vec<_>, Vec<_>) =
      std::mem::take(&mut self.entries)
        .into_values()
        .flatten()
        .partition(|item| item.kind.is_user_defined());
    user_defined_entries.iter().for_each(|entry| {
      let module = match &self.module_table[entry.idx] {
        Module::Normal(module) => module,
        Module::External(_module) => {
          // Case: import('external').
          return;
        }
      };
      context.bailout_cjs_tree_shaking_modules.insert(module.idx);
      entry_export_roots.get(entry.idx).unwrap_or_default().iter().for_each(|root| {
        let symbol_ref = root.symbol_ref;
        if let Module::Normal(_) = &context.modules[symbol_ref.owner] {
          include_declaring_statements(context, &symbol_ref);
          include_symbol_and_check_cjs_bailout(
            context,
            symbol_ref,
            SymbolIncludeReason::EntryExport,
          );
        }
      });
      include_module(context, module);
    });

    let mut unused_record_idxs = vec![];
    let cycled_idx = self.sort_dynamic_entries_by_topological_order(&mut dynamic_entries);
    let mut included_dynamic_entry = FxHashSet::default();
    loop {
      context.module_inclusion_changed = false;

      // It could be safely take since it is no more used.
      // We extract bailout_modules first to avoid borrowing conflict:
      // passing `context` requires a mutable borrow, which conflicts with
      // borrowing `context.bailout_cjs_tree_shaking_modules` inside the call.
      let bailout_modules = std::mem::take(&mut context.bailout_cjs_tree_shaking_modules);
      include_cjs_bailout_exports(context, &self.metas, bailout_modules);

      dynamic_entries.iter().for_each(|entry| {
        if included_dynamic_entry.contains(&entry.idx) {
          return;
        }
        let included = self.process_and_retain_dynamic_entry(
          entry,
          &cycled_idx,
          context,
          &mut unused_record_idxs,
          unreachable_import_expression_node_ids,
          entry_export_roots,
        );
        if included {
          included_dynamic_entry.insert(entry.idx);
        }
      });

      if !context.module_inclusion_changed {
        break;
      }
    }

    // Under `preserveModules`, preserve each module's re-exports whose canonical value survived
    // tree-shaking, so every emitted file mirrors its source's export interface (issue #9122).
    preserve_reexported_interfaces(context);

    dynamic_entries.retain(|entry| included_dynamic_entry.contains(&entry.idx));

    // update entries with lived only.
    self.entries = {
      let mut entries = FxIndexMap::default();
      for entry in
        user_defined_entries.into_iter().chain(if self.options.code_splitting.is_disabled() {
          itertools::Either::Left(std::iter::empty())
        } else {
          itertools::Either::Right(dynamic_entries.into_iter())
        })
      {
        entries.entry(entry.idx).or_insert_with(Vec::new).push(entry);
      }
      entries
    };

    // Setting the json module none self reference included symbol map
    for (mi, set) in std::mem::take(&mut context.json_module_none_self_reference_included_symbol) {
      let module = self.module_table[mi].as_normal_mut().expect("should be a normal module");
      _ = module.ecma_view.json_module_none_self_reference_included_symbol.insert(Box::new(set));
    }

    // mark those dynamic import records as dead, in case we could eliminate them later in ast
    // visitor.
    for (mi, record_idx) in unused_record_idxs {
      let module = self.module_table[mi].as_normal_mut().expect("should be a normal module");
      let rec = &mut module.import_records[record_idx];
      rec.meta.insert(ImportRecordMeta::DeadDynamicImport);
    }

    self
      .module_table
      .modules
      .par_iter_mut()
      .zip_eq(self.metas.par_iter_mut())
      .zip_eq(statement_runtime_requirements.slots().par_iter())
      .filter_map(|((m, meta), depended_helper)| {
        m.as_normal_mut().map(|m| (m, meta, depended_helper))
      })
      .for_each(|(module, meta, depended_helper)| {
        let idx = module.idx;
        let mut normalized_runtime_helper = RuntimeHelper::default();
        for (helper, stmt_info_idxs) in depended_helper.iter() {
          if stmt_info_idxs.is_empty() {
            continue;
          }
          let any_included = stmt_info_idxs
            .iter()
            .any(|stmt_info_idx| is_stmt_info_included_vec[module.idx].has_bit(*stmt_info_idx));
          // We also need to process the runtime helper of an eliminated module so that we
          // can propagate it to its importers later.
          normalized_runtime_helper.set(
            helper,
            any_included
              || (module.id != RUNTIME_MODULE_ID && !is_module_included_vec.has_bit(idx)),
          );
        }
        meta.depended_runtime_helper = normalized_runtime_helper;
        meta.module_namespace_included_reason = module_namespace_included_reason[module.idx];
      });

    let depended_runtime_helper = collect_depended_runtime_helpers(
      &self.module_table.modules,
      &self.metas,
      &is_module_included_vec,
    );
    let context = &mut IncludeContext::new(
      &self.module_table.modules,
      &self.stmt_infos,
      &self.symbols,
      &mut is_stmt_info_included_vec,
      &mut is_module_included_vec,
      self.runtime.id(),
      &self.metas,
      &mut used_symbol_refs,
      &mut used_external_symbols,
      &self.global_constant_symbol_map,
      self.options,
      &self.normal_symbol_exports_chain_map,
      &mut module_namespace_included_reason,
      &entry_module_idxs,
      &body_demand_keys,
    );
    include_runtime_symbol(context, &self.runtime, depended_runtime_helper);

    self.used_symbol_refs = used_symbol_refs;
    self.used_external_symbols = used_external_symbols;
    // Store the final statement inclusion results back to metas.
    is_stmt_info_included_vec.into_iter_enumerated().for_each(|(module_idx, stmt_included_vec)| {
      self.metas[module_idx].stmt_info_included = stmt_included_vec;
    });
    // Store the final module inclusion results back to metas.
    for (module_idx, meta) in self.metas.iter_mut_enumerated() {
      meta.is_included = is_module_included_vec.has_bit(module_idx);
    }

    tracing::trace!(
      "included statements {:#?}",
      self
        .module_table
        .modules
        .iter()
        .filter_map(Module::as_normal)
        .map(|m| m.to_debug_normal_module_for_tree_shaking(
          &self.stmt_infos[m.idx],
          self.metas[m.idx].is_included,
          &self.metas[m.idx].stmt_info_included
        ))
        .collect::<Vec<_>>()
    );
  }
}

/// Public-entry variant of [`enqueue_declaring_statements`]: include every statement that
/// declares `symbol_ref` in its owner module, draining the queue like the other `include_*`
/// entry points. Use this from outside the engine (driver, dynamic entries); handlers use the
/// enqueue-only variant.
pub(super) fn include_declaring_statements(ctx: &mut IncludeContext, symbol_ref: &SymbolRef) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  enqueue_declaring_statements(ctx, symbol_ref);
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

/// Enqueue every statement that declares `symbol_ref` in its owner module (no-op for external
/// owners). This is the "a used binding keeps its declaration" edge, applied at every reference
/// site. Engine-internal: enqueues without draining.
fn enqueue_declaring_statements(ctx: &mut IncludeContext, symbol_ref: &SymbolRef) {
  if let Module::Normal(_) = &ctx.modules[symbol_ref.owner] {
    ctx.stmt_infos[symbol_ref.owner].declared_stmts_by_symbol(symbol_ref).iter().copied().for_each(
      |stmt_info_id| {
        ctx.pending.push(WorkItem::Statement(symbol_ref.owner, stmt_info_id));
      },
    );
  }
}

pub fn include_module(ctx: &mut IncludeContext, module: &NormalModule) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  ctx.pending.push(WorkItem::Module(module.idx));
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

fn handle_include_module(ctx: &mut IncludeContext, module_idx: ModuleIdx) {
  let Module::Normal(module) = &ctx.modules[module_idx] else {
    return;
  };
  if !ctx.is_module_included_vec.set_bit(module.idx) {
    return;
  }
  ctx.module_inclusion_changed = true;

  if module.idx == ctx.runtime_idx && !module.side_effects.has_side_effects() {
    // Unmodified runtime: statements included only via references.
    return;
  }

  let forced_no_treeshake = matches!(module.side_effects, DeterminedSideEffects::NoTreeshake);
  if ctx.tree_shaking && !forced_no_treeshake {
    sweep_side_effect_statements(ctx, module);
  } else {
    include_statements_without_treeshaking(ctx, module);
  }

  include_side_effectful_dependencies(ctx, module);

  if module.meta.has_eval() && matches!(module.module_type, ModuleType::Js | ModuleType::Jsx) {
    // `eval` can observe any module-level binding, so every import must survive.
    module.named_imports.keys().for_each(|symbol| {
      push_symbol_and_check_cjs_bailout(ctx, *symbol, SymbolIncludeReason::Normal);
    });
  }

  ctx.metas[module.idx].included_commonjs_export_symbol.iter().for_each(|symbol_ref| {
    push_symbol_and_check_cjs_bailout(ctx, *symbol_ref, SymbolIncludeReason::Normal);
  });

  // With enabling HMR, rolldown will register included esm module's namespace object to the runtime.
  if ctx.options.is_dev_mode_enabled()
    && module.idx != ctx.runtime_idx
    && matches!(module.exports_kind, ExportsKind::Esm)
  {
    ctx.pending.push(WorkItem::Statement(module.idx, StmtInfos::NAMESPACE_STMT_IDX));
    ctx.module_namespace_included_reason[module.idx].insert(ModuleNamespaceIncludedReason::Unknown);
  }
}

/// The unconditional side-effect sweep of an included module under tree shaking: force-include
/// every top-level statement that evaluates side effects (plus `eval` bail statements).
///
/// Binding-reading side-effect statements of a user-declared side-effect-free module join only
/// through body demand instead of this sweep; see `side_effects_included_on_demand`. Statements
/// without symbol references (e.g. a bare `console.log()`) and import/re-export statements cannot
/// dangle an import and keep today's behavior.
fn sweep_side_effect_statements(ctx: &mut IncludeContext, module: &NormalModule) {
  let on_demand_side_effects = side_effects_included_on_demand(module, ctx.entry_module_idxs);
  ctx.stmt_infos[module.idx].iter_enumerated_without_namespace_stmt().for_each(
    |(stmt_info_id, stmt_info)| {
      // No need to handle the namespace statement specially, because it doesn't have side effects and will only be included if it is used.
      let bail_eval = module.meta.has_eval()
        && !stmt_info.declared_symbols.is_empty()
        && stmt_info_id.index() != 0;
      let has_side_effects = if module.meta.contains(EcmaViewMeta::SafelyTreeshakeCommonjs)
        && ctx.options.treeshake.commonjs()
      {
        stmt_info.eval_flags.contains(StmtEvalFlags::UnknownSideEffect)
      } else {
        stmt_info.eval_flags.has_side_effect_for_tree_shaking()
      };
      if (has_side_effects && !(on_demand_side_effects && is_gated_side_effect_stmt(stmt_info)))
        || bail_eval
      {
        ctx.pending.push(WorkItem::Statement(module.idx, stmt_info_id));
      }
    },
  );
}

/// With tree shaking disabled (or `moduleSideEffects: "no-treeshake"`), every statement of an
/// included module is kept, except `force_tree_shaking` statements which still join only via side
/// effects or references.
fn include_statements_without_treeshaking(ctx: &mut IncludeContext, module: &NormalModule) {
  // Skip the namespace statement. It should be included only if it is used no matter tree shaking is enabled or not.
  ctx.stmt_infos[module.idx].iter_enumerated_without_namespace_stmt().for_each(
    |(stmt_info_id, stmt_info)| {
      if stmt_info.force_tree_shaking {
        if stmt_info.eval_flags.has_side_effect_for_tree_shaking() {
          // If `force_tree_shaking` is true, the statement should be included either by itself having side effects
          // or by other statements referencing it.
          ctx.pending.push(WorkItem::Statement(module.idx, stmt_info_id));
        }
      } else {
        ctx.pending.push(WorkItem::Statement(module.idx, stmt_info_id));
      }
    },
  );
}

/// Include imported modules for their side effects: an included module evaluates each dependency
/// that has (or may have) side effects, even when none of its bindings are used.
fn include_side_effectful_dependencies(ctx: &mut IncludeContext, module: &NormalModule) {
  let module_meta = &ctx.metas[module.idx];

  module_meta.dependencies.iter().copied().for_each(|dependency_idx| {
    // Guard-hoist: skip already-included dependencies before paying the
    // `ctx.modules[idx]` match + `has_side_effects()` check. The authoritative
    // dedup is still `set_bit` inside `include_module`; this is a pure work-skip.
    if ctx.is_module_included_vec.has_bit(dependency_idx) {
      return;
    }
    match &ctx.modules[dependency_idx] {
      Module::Normal(importee) => {
        if !ctx.tree_shaking || importee.side_effects.has_side_effects() {
          ctx.pending.push(WorkItem::Module(importee.idx));
        }
      }
      Module::External(_) => {}
    }
  });
  tracing::trace!(
    "{}:\n module_meta dependencies: {:#?}",
    module.stable_id,
    module_meta.dependencies.iter().map(|idx| { ctx.modules[*idx].id().to_string() }).collect_vec()
  );
}

pub fn include_symbol(
  ctx: &mut IncludeContext,
  symbol_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  ctx.pending.push(WorkItem::Symbol(symbol_ref, include_reason));
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

fn handle_include_symbol(
  ctx: &mut IncludeContext,
  symbol_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) {
  let mut canonical_ref = ctx.symbols.canonical_ref_for(symbol_ref);

  if is_bypassed_inlined_constant(ctx, canonical_ref, include_reason) {
    return;
  }

  drain_body_demand_stmts(ctx, canonical_ref);

  // Also include the symbol that points to the canonical ref.
  ctx.used_symbol_refs.insert(symbol_ref);
  if ctx.modules[symbol_ref.owner].is_external() {
    ctx.used_external_symbols.insert(symbol_ref);
  }

  // CJS bailout checks are handled by `include_symbol_and_check_cjs_bailout`
  // at most call sites. This keeps `include_symbol` focused on inclusion only.

  follow_cjs_namespace_alias(ctx, &mut canonical_ref);

  let is_simulated_facade_chunk =
    note_namespace_inclusion_reason(ctx, canonical_ref, include_reason);

  ctx.used_symbol_refs.insert(canonical_ref);
  if ctx.modules[canonical_ref.owner].is_external() {
    ctx.used_external_symbols.insert(canonical_ref);
  }
  if let Module::Normal(module) = &ctx.modules[canonical_ref.owner] {
    demand_esm_init_wrapper(ctx, canonical_ref);
    note_json_self_reference(ctx, module, canonical_ref, include_reason);
    enqueue_declaring_statements(ctx, &canonical_ref);
    if !is_simulated_facade_chunk {
      ctx.pending.push(WorkItem::Module(module.idx));
    }
  }

  include_property_write_referencing_stmts(ctx, symbol_ref);
}

/// Mirror of the finalizer's constant inlining: a constant value is always inlined at its
/// reference sites, so the reference does not retain the declaration.
///
/// If the symbol is a constant value and it is not a commonjs module export, we don't need to
/// include it since it would be always inlined. In smart mode, we only skip if `safe_to_inline`
/// is true (meaning it will be inlined regardless of context). We don't need to add any flag
/// since if `inlineConst` is disabled, the test expr will always return `false`.
fn is_bypassed_inlined_constant(
  ctx: &IncludeContext,
  canonical_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) -> bool {
  if let Some(v) = ctx.constant_symbol_map.get(&canonical_ref)
    && !include_reason.contains(SymbolIncludeReason::EntryExport)
    && (!ctx.inline_const_smart || v.safe_to_inline)
    && !v.commonjs_export
  {
    return true;
  }
  false
}

/// Demanding a user-declared side-effect-free module's own export (or its
/// namespace) makes the module's body observable, so its gated side-effect
/// statements join now (e.g. `foo.bar = 1` once `foo` is demanded); see
/// `side_effects_included_on_demand`. Must sit after the inlined-constant
/// bypass: demand satisfied by inlining doesn't include the module today.
/// The `body_demand_swept` bit keeps each module swept at most once.
fn drain_body_demand_stmts(ctx: &mut IncludeContext, canonical_ref: SymbolRef) {
  let Some(&module_idx) = ctx.body_demand_keys.get(&canonical_ref) else {
    return;
  };
  if !ctx.body_demand_swept.insert(module_idx) {
    return;
  }
  if let Module::Normal(_) = &ctx.modules[module_idx] {
    ctx.stmt_infos[module_idx].iter_enumerated_without_namespace_stmt().for_each(
      |(stmt_info_idx, stmt_info)| {
        if is_gated_side_effect_stmt(stmt_info) {
          ctx.pending.push(WorkItem::Statement(module_idx, stmt_info_idx));
        }
      },
    );
  }
}

/// Follow the CJS-interop alias: the canonical of `import { a } from './cjs.js'` points at the
/// interop namespace binding (`import_cjs`) via `namespace_alias`. Rewrites `canonical_ref` to
/// the alias target and includes the specific named export of the CJS module.
fn follow_cjs_namespace_alias(ctx: &mut IncludeContext, canonical_ref: &mut SymbolRef) {
  let canonical_ref_symbol = ctx.symbols.get(*canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    *canonical_ref = namespace_alias.namespace_ref;
    if let Some(idx) =
      ctx.metas[canonical_ref.owner].import_record_ns_to_cjs_module.get(canonical_ref)
    {
      // Include specific named export from CJS module.
      // Default import bailout is handled by check_cjs_bailout at call sites.
      // ```js
      // import {a} from './cjs.js'
      // console.log(a)
      // ```
      ctx.modules[*idx].as_normal().inspect(|_| {
        let Some(export_symbol) =
          ctx.metas[*idx].resolved_exports.get(&namespace_alias.property_name)
        else {
          return;
        };
        if namespace_alias.property_name.as_str() != "default" {
          ctx.pending.push(WorkItem::Symbol(export_symbol.symbol_ref, SymbolIncludeReason::Normal));
        }
      });
    }
  }
}

/// When the canonical is a module-namespace object, record *why* the namespace is included (the
/// finalizer emits it differently per reason). Returns whether this inclusion originates from a
/// simulated facade chunk, in which case the owner module itself must not be structurally
/// included.
fn note_namespace_inclusion_reason(
  ctx: &mut IncludeContext,
  canonical_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) -> bool {
  let is_module_namespace =
    ctx.modules[canonical_ref.owner].namespace_object_ref() == Some(canonical_ref);
  if is_module_namespace {
    if include_reason.intersects(SymbolIncludeReason::Normal | SymbolIncludeReason::EntryExport) {
      ctx.module_namespace_included_reason[canonical_ref.owner]
        .insert(ModuleNamespaceIncludedReason::Unknown);
    } else if include_reason.contains(SymbolIncludeReason::ReExportDynamicExports) {
      ctx.module_namespace_included_reason[canonical_ref.owner]
        .insert(ModuleNamespaceIncludedReason::ReExportDynamicExports);
    }
    include_reason.intersects(SymbolIncludeReason::SimulatedFacadeChunk)
  } else {
    false
  }
}

/// Using any binding of a `WrapKind::Esm` module demands its `init_*` wrapper: the binding is
/// only initialized once the wrapper runs.
fn demand_esm_init_wrapper(ctx: &mut IncludeContext, canonical_ref: SymbolRef) {
  let wrapper_ref = {
    let meta = &ctx.metas[canonical_ref.owner];
    matches!(meta.wrap_kind(), WrapKind::Esm)
      .then_some(meta.wrapper_ref)
      .flatten()
      .filter(|wrapper_ref| *wrapper_ref != canonical_ref)
  };
  if let Some(wrapper_ref) = wrapper_ref {
    ctx.pending.push(WorkItem::Symbol(wrapper_ref, SymbolIncludeReason::Normal));
  }
}

/// Track which of a JSON module's top-level properties are referenced from *outside* its own
/// synthesized default export, so the finalizer knows which properties cannot be inlined.
fn note_json_self_reference(
  ctx: &mut IncludeContext,
  module: &NormalModule,
  canonical_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) {
  if !include_reason.contains(SymbolIncludeReason::JsonDefaultExportSelfReference)
    && module.module_type == ModuleType::Json
  {
    ctx
      .json_module_none_self_reference_included_symbol
      .entry(module.idx)
      .or_default()
      .insert(canonical_ref);
  }
}

/// With `propertyWriteSideEffects: false`, property-write statements are not side effects — but
/// once the written-to symbol is included, its write statements must come along.
fn include_property_write_referencing_stmts(ctx: &mut IncludeContext, symbol_ref: SymbolRef) {
  if matches!(
    ctx.options.treeshake.property_write_side_effects(),
    rolldown_common::PropertyWriteSideEffects::False
  ) {
    let stmt_ids: &[StmtInfoIdx] = ctx.stmt_infos[symbol_ref.owner]
      .symbol_ref_to_referenced_stmt_idx()
      .get(&symbol_ref)
      .map(Vec::as_slice)
      .unwrap_or(&[]);
    if ctx.modules[symbol_ref.owner].as_normal().is_some() {
      for stmt_info_id in stmt_ids.iter().copied() {
        ctx.pending.push(WorkItem::Statement(symbol_ref.owner, stmt_info_id));
      }
    }
  }
}

pub fn include_statement(
  ctx: &mut IncludeContext,
  module: &NormalModule,
  stmt_info_idx: StmtInfoIdx,
) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  ctx.pending.push(WorkItem::Statement(module.idx, stmt_info_idx));
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

fn handle_include_statement(
  ctx: &mut IncludeContext,
  module_idx: ModuleIdx,
  stmt_info_idx: StmtInfoIdx,
) {
  let Module::Normal(module) = &ctx.modules[module_idx] else {
    return;
  };
  // include the statement itself
  if !ctx.is_included_vec[module.idx].set_bit(stmt_info_idx) {
    return;
  }

  let stmt_info = ctx.stmt_infos[module.idx].get(stmt_info_idx);

  scan_import_records_for_cjs_bailout(ctx, module, stmt_info);
  let mut include_kind = if stmt_info.meta.contains(StmtInfoMeta::ReExportDynamicExports) {
    SymbolIncludeReason::ReExportDynamicExports
  } else {
    SymbolIncludeReason::Normal
  };

  let is_json_module = module.module_type == ModuleType::Json;

  // For a transformed json module
  if is_json_module && !stmt_info.referenced_symbols.is_empty() {
    include_kind |= SymbolIncludeReason::JsonDefaultExportSelfReference;
  }

  stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
    if let Some(member_expr_resolution) = match reference_ref {
      SymbolOrMemberExprRef::Symbol(_) => None,
      SymbolOrMemberExprRef::MemberExpr(member_expr_ref) => {
        member_expr_ref.resolution(&ctx.metas[module.idx].resolved_member_expr_refs)
      }
    } {
      // Caveat: If we can get the `MemberExprRefResolution` from the `resolved_member_expr_refs`,
      // it means this member expr definitely contains module namespace ref.
      if let Some(resolved_ref) = member_expr_resolution.resolved {
        member_expr_resolution.depended_refs.iter().for_each(|sym_ref| {
          enqueue_declaring_statements(ctx, sym_ref);
        });
        ctx.pending.push(WorkItem::Symbol(resolved_ref, include_kind));
        // When the member expression resolves to a specific CJS export property
        // (e.g., `ns.x`), we skip the bailout check — we know the access is partial
        // and CJS tree-shaking can work. Otherwise, the full namespace may be used
        // opaquely, so we check for bailout.
        if member_expr_resolution.target_commonjs_exported_symbol.is_none() {
          check_cjs_bailout(ctx, resolved_ref);
        }
      } else {
        // If it points to nothing, the expression will be rewritten as `void 0` and there's nothing we need to include
      }
    } else {
      // For enum member accesses (e.g., `B.member`), check if the member will be inlined
      // by the finalizer. If so, skip including the enum's declaration — it's dead code
      // after inlining. This mirrors the `constant_symbol_map` bypass in `include_symbol`.
      if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = reference_ref {
        if is_inlined_enum_member_access(ctx.modules, ctx.symbols, member_expr_ref) {
          return;
        }
      }
      let original_ref = reference_ref.symbol_ref();
      std::iter::once(original_ref)
        .chain(
          ctx.normal_symbol_exports_chain_map.get(original_ref).map(Vec::as_slice).unwrap_or(&[]),
        )
        .for_each(|sym_ref| {
          enqueue_declaring_statements(ctx, sym_ref);
        });
      push_symbol_and_check_cjs_bailout(ctx, *original_ref, include_kind);
    }
  });
}

/// FIXME: bailout for require() import for now
/// it is fine for now, since webpack did not support it either
/// ```js
/// const cjs = require('./cjs.js')
/// ```
fn scan_import_records_for_cjs_bailout(
  ctx: &mut IncludeContext,
  module: &NormalModule,
  stmt_info: &StmtInfo,
) {
  stmt_info
    .import_records
    .iter()
    .filter_map(|import_record_idx| {
      let rec = &module.import_records[*import_record_idx];
      rec.resolved_module.map(|module_idx| (rec, module_idx))
    })
    .for_each(|(import_record, module_idx)| {
      let Some(m) = ctx.modules[module_idx].as_normal() else {
        // If the import record is not a normal module, we don't need to include it.
        return;
      };
      if !matches!(m.exports_kind, ExportsKind::CommonJs)
        || import_record.kind == ImportKind::Import
      {
        return;
      }
      // Skip CJS bailout for dynamic imports that will be determined dead:
      // top-level pure (unused exports) importing a side-effect-free module.
      // The dynamic entry mechanism handles CJS bailout for live entries via
      // `process_and_retain_dynamic_entry`. Without this check, a dead dynamic
      // import's CJS bailout would mark the module as included while the entry
      // is later removed, causing an empty-bits assertion in code splitting.
      if import_record.meta.contains(ImportRecordMeta::TopLevelPureDynamicImport)
        && !m.side_effects.has_side_effects()
      {
        return;
      }
      if module.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport) {
        // When the importer has multiple CJS re-export targets (conditional re-exports),
        // bail out to prevent tree-shaking from dropping any branch's exports.
        if module.ecma_view.cjs_reexport_import_record_ids.len() > 1 {
          ctx.bailout_cjs_tree_shaking_modules.insert(module_idx);
        }
      } else {
        ctx.bailout_cjs_tree_shaking_modules.insert(module_idx);
      }
    });
}

/// Whether a member access will be inlined as an enum member literal by the finalizer, in which
/// case the enum declaration must not be retained by this reference.
///
/// This applies to both const and regular enums. The member access will be replaced
/// by a literal, so the reference no longer needs the declaration. If the enum is
/// also referenced as a bare symbol (e.g., `typeof E`, `console.log(E)`), that
/// separate reference will independently include the declaration via `include_symbol`.
/// Regular enum IIFEs are `@__PURE__`, so they'll be tree-shaken if truly unused.
/// Enum inlining is unconditional (not gated by inlineConst mode) because it implements
/// TypeScript's const enum semantics, which mandate replacement. Only simple member accesses
/// (e.g. `E.member`) are inlined — not deep chains like `E.member.something`, and not writes.
fn is_inlined_enum_member_access(
  modules: &IndexModules,
  symbols: &SymbolRefDb,
  member_expr_ref: &MemberExprRef,
) -> bool {
  let canonical_ref = symbols.canonical_ref_for(member_expr_ref.object_ref);
  let Some(Module::Normal(owner_module)) = modules.get(canonical_ref.owner) else {
    return false;
  };
  let symbol_name = canonical_ref.name(symbols);
  let Some(members) = owner_module.ecma_view.enum_member_value_map.get(symbol_name) else {
    return false;
  };
  !member_expr_ref.is_write
    && matches!(member_expr_ref.prop_and_span_list.as_slice(),
      [prop] if members.contains_key(prop.name.as_str()))
}
