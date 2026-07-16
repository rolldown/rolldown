use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  ConstExportMeta, EcmaModuleAstUsage, EcmaViewMeta, ExportOrigin, ExportsKind, ImportKind,
  ImportRecordMeta, IndexModules, MemberExprRef, MemberExprRefResolution, Module, ModuleIdx,
  ModuleNamespaceIncludedReason, ModuleType, NormalModule, PropertyWriteSideEffects,
  RUNTIME_HELPER_NAMES, RuntimeHelper, RuntimeModuleBrief, StmtEvalFlags, StmtInfo, StmtInfoIdx,
  StmtInfoMeta, StmtInfos, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb, UsedExternalSymbols,
  UsedSymbolRefsBuilder, side_effects::DeterminedSideEffects,
};
use rolldown_utils::IndexBitSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::type_alias::IndexStmtInfos;

pub type StmtInclusionVec = IndexVec<ModuleIdx, IndexBitSet<StmtInfoIdx>>;
pub type ModuleInclusionVec = IndexBitSet<ModuleIdx>;
pub type ModuleNamespaceReasonVec = IndexVec<ModuleIdx, ModuleNamespaceIncludedReason>;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SymbolIncludeReason: u8 {
        const Normal = 1;
        const EntryExport = 1 << 1;
        /// Set for the normal-import and normal-dynamic-import branches in
        /// `passes/reference_needed_symbols.rs` when exports are resolved dynamically.
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

/// Final module facts read by both body-demand discovery and the inclusion algorithm.
pub(in crate::stages::link_stage) trait InclusionModuleFacts {
  fn exports_kind(&self, module_idx: ModuleIdx) -> ExportsKind;

  fn side_effects(&self, module_idx: ModuleIdx) -> DeterminedSideEffects;
}

/// The remaining immutable linking facts read by the inclusion algorithm.
///
/// Each method exposes one semantic query instead of a stage or metadata carrier. The legacy
/// adapter answers these queries from compatibility data; the Link pass can later answer the same
/// queries directly from its typed artifacts without changing the algorithm.
pub(in crate::stages::link_stage) trait InclusionFacts:
  InclusionModuleFacts
{
  fn cjs_namespace_target(
    &self,
    importer_idx: ModuleIdx,
    namespace_ref: SymbolRef,
  ) -> Option<ModuleIdx>;

  fn resolved_export_symbol(&self, module_idx: ModuleIdx, name: &str) -> Option<SymbolRef>;

  fn commonjs_export_symbols(&self, module_idx: ModuleIdx) -> impl Iterator<Item = SymbolRef> + '_;

  fn included_commonjs_export_symbols(
    &self,
    module_idx: ModuleIdx,
  ) -> impl Iterator<Item = SymbolRef> + '_;

  fn dependencies(&self, module_idx: ModuleIdx) -> impl Iterator<Item = ModuleIdx> + '_;

  fn member_expr_resolution<'a>(
    &'a self,
    module_idx: ModuleIdx,
    member_expr_ref: &MemberExprRef,
  ) -> Option<&'a MemberExprRefResolution>;

  fn esm_wrapper_ref(&self, module_idx: ModuleIdx) -> Option<SymbolRef>;

  fn constant_export(&self, symbol_ref: &SymbolRef) -> Option<&ConstExportMeta>;

  fn normal_export_chain(&self, symbol_ref: &SymbolRef) -> &[SymbolRef];

  fn normal_export_chains(&self) -> impl Iterator<Item = (SymbolRef, &[SymbolRef])> + '_;
}

/// Configuration bits that are actually observed by the inclusion algorithm.
#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct TreeShakingConfig {
  pub enabled: bool,
  pub commonjs: bool,
  pub property_write_side_effects: PropertyWriteSideEffects,
}

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct InclusionConfig {
  pub tree_shaking: TreeShakingConfig,
  pub inline_const_smart: bool,
  pub preserve_modules: bool,
  pub dev_mode: bool,
}

/// The complete, short-lived execution context for the shared inclusion algorithm.
///
/// This context contains only the tables, exact fact interface, and mutable worksets used by one
/// inclusion run. It deliberately cannot carry stage or metadata aggregates, or full bundler
/// options. Legacy callers reborrow their existing state into it for one entry-point call.
pub(in crate::stages::link_stage) struct InclusionCoreContext<'a, F: InclusionFacts> {
  pub facts: &'a F,
  pub modules: &'a IndexModules,
  pub stmt_infos: &'a IndexStmtInfos,
  pub symbols: &'a SymbolRefDb,
  pub is_included_vec: &'a mut StmtInclusionVec,
  pub is_module_included_vec: &'a mut ModuleInclusionVec,
  pub config: InclusionConfig,
  pub runtime_idx: ModuleIdx,
  pub used_symbol_refs: &'a mut UsedSymbolRefsBuilder,
  pub used_external_symbols: &'a mut UsedExternalSymbols,
  pub bailout_cjs_tree_shaking_modules: &'a mut FxHashSet<ModuleIdx>,
  pub module_inclusion_changed: &'a mut bool,
  pub module_namespace_included_reason: &'a mut ModuleNamespaceReasonVec,
  pub json_module_none_self_reference_included_symbol:
    &'a mut FxHashMap<ModuleIdx, FxHashSet<SymbolRef>>,
  pub entry_module_idxs: &'a FxHashSet<ModuleIdx>,
  pub body_demand_keys: &'a FxHashMap<SymbolRef, ModuleIdx>,
  pub body_demand_swept: &'a mut FxHashSet<ModuleIdx>,
  pub pending: &'a mut Vec<WorkItem>,
}

/// A unit of inclusion work. Edge producers push these instead of recursing, which keeps the
/// engine iterative (no stack-depth limit on deep graphs) and makes the traversal a visible
/// queue rather than an implicit call stack. Visible outside this module only as the legacy
/// `IncludeContext::pending` field's type.
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

/// Compute the body-demand keys from the final module format and side-effect facts.
///
/// The public legacy wrapper delegates here today; the future typed Link path can call the same
/// core after its explicit artifacts replace the compatibility metadata reads.
pub(in crate::stages::link_stage) fn compute_body_demand_keys_core<F: InclusionModuleFacts>(
  facts: &F,
  modules: &IndexModules,
  stmt_infos: &IndexStmtInfos,
  symbols: &SymbolRefDb,
  treeshake_enabled: bool,
  entry_module_idxs: &FxHashSet<ModuleIdx>,
) -> FxHashMap<SymbolRef, ModuleIdx> {
  let mut map: FxHashMap<SymbolRef, ModuleIdx> = FxHashMap::default();
  if !treeshake_enabled {
    return map;
  }
  for module in modules.iter().filter_map(Module::as_normal) {
    if !side_effects_included_on_demand_for(
      module,
      entry_module_idxs,
      facts.side_effects(module.idx),
      facts.exports_kind(module.idx),
    ) {
      continue;
    }
    let has_gated_stmts = stmt_infos[module.idx]
      .iter_enumerated_without_namespace_stmt()
      .any(|(_, stmt_info)| is_gated_side_effect_stmt(stmt_info));
    if !has_gated_stmts {
      continue;
    }
    let body_demand_keys = module
      .named_exports
      .values()
      .filter(|local_export| matches!(module.classify_export(local_export), ExportOrigin::Own))
      .map(|local_export| symbols.canonical_ref_for(local_export.referenced))
      .chain(std::iter::once(module.namespace_object_ref));
    for key in body_demand_keys {
      let previous = map.insert(key, module.idx);
      debug_assert!(
        previous.is_none_or(|prev| prev == module.idx),
        "a body-demand key must belong to exactly one module"
      );
    }
  }
  map
}

fn side_effects_included_on_demand_for(
  module: &NormalModule,
  entry_module_idxs: &FxHashSet<ModuleIdx>,
  side_effects: DeterminedSideEffects,
  exports_kind: ExportsKind,
) -> bool {
  matches!(side_effects, DeterminedSideEffects::UserDefined(false))
    && matches!(exports_kind, ExportsKind::Esm)
    && !module.meta.has_eval()
    && !entry_module_idxs.contains(&module.idx)
}

pub(super) fn is_gated_side_effect_stmt(stmt_info: &StmtInfo) -> bool {
  stmt_info.eval_flags.has_side_effect_for_tree_shaking()
    && !stmt_info.referenced_symbols.is_empty()
    && stmt_info.import_records.is_empty()
}

/// Drain the work queue to empty. LIFO order mirrors the depth-first shape of the recursion this
/// engine replaced; the final inclusion sets are a monotone closure over the pushed edges, so
/// drain order affects only traversal order, never the result.
fn drain_work_items<F: InclusionFacts>(ctx: &mut InclusionCoreContext<'_, F>) {
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
/// Use this at most call sites. Only use bare [`include_symbol`] when you explicitly want to skip
/// the bailout check (for example, partial CJS member-expression access or runtime symbols).
pub(in crate::stages::link_stage) fn include_symbol_and_check_cjs_bailout<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
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
fn push_symbol_and_check_cjs_bailout<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
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
fn check_cjs_bailout<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  symbol_ref: SymbolRef,
) {
  let canonical_ref = ctx.symbols.canonical_ref_for(symbol_ref);

  // If the symbol is a CJS namespace import ref, bail out the target CJS module.
  if let Some(idx) = ctx.facts.cjs_namespace_target(canonical_ref.owner, canonical_ref) {
    ctx.bailout_cjs_tree_shaking_modules.insert(idx);
  }
  // If the symbol IS a CJS module's namespace object, bail out that module.
  if ctx.modules[canonical_ref.owner].namespace_object_ref() == Some(canonical_ref) {
    ctx.bailout_cjs_tree_shaking_modules.insert(canonical_ref.owner);
  }

  // If the symbol has a namespace_alias importing "default" from a CJS module,
  // bail out that module (default import is the whole module.exports).
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias
    && let Some(idx) = ctx
      .facts
      .cjs_namespace_target(namespace_alias.namespace_ref.owner, namespace_alias.namespace_ref)
    && namespace_alias.property_name.as_str() == "default"
  {
    ctx.bailout_cjs_tree_shaking_modules.insert(idx);
  }
}

/// Public-entry variant of [`enqueue_declaring_statements`]: include every statement that
/// declares `symbol_ref` in its owner module, draining the queue like the other `include_*`
/// entry points. Use this from outside the engine; handlers use the enqueue-only variant.
pub(in crate::stages::link_stage) fn include_declaring_statements<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  symbol_ref: &SymbolRef,
) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  enqueue_declaring_statements(ctx, symbol_ref);
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

/// Enqueue every statement that declares `symbol_ref` in its owner module (no-op for external
/// owners). This is the "a used binding keeps its declaration" edge, applied at every reference
/// site. Engine-internal: enqueues without draining.
fn enqueue_declaring_statements<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  symbol_ref: &SymbolRef,
) {
  if let Module::Normal(_) = &ctx.modules[symbol_ref.owner] {
    ctx.stmt_infos[symbol_ref.owner].declared_stmts_by_symbol(symbol_ref).iter().copied().for_each(
      |stmt_info_id| {
        ctx.pending.push(WorkItem::Statement(symbol_ref.owner, stmt_info_id));
      },
    );
  }
}

pub(in crate::stages::link_stage) fn include_module<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  module: &NormalModule,
) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  ctx.pending.push(WorkItem::Module(module.idx));
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

fn handle_include_module<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  module_idx: ModuleIdx,
) {
  let Module::Normal(module) = &ctx.modules[module_idx] else {
    return;
  };
  if !ctx.is_module_included_vec.set_bit(module.idx) {
    return;
  }
  *ctx.module_inclusion_changed = true;

  if module.idx == ctx.runtime_idx && !ctx.facts.side_effects(module.idx).has_side_effects() {
    // Unmodified runtime: statements included only via references.
    return;
  }

  let forced_no_treeshake =
    matches!(ctx.facts.side_effects(module.idx), DeterminedSideEffects::NoTreeshake);
  if ctx.config.tree_shaking.enabled && !forced_no_treeshake {
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

  let facts = ctx.facts;
  facts.included_commonjs_export_symbols(module.idx).for_each(|symbol_ref| {
    push_symbol_and_check_cjs_bailout(ctx, symbol_ref, SymbolIncludeReason::Normal);
  });

  // With enabling HMR, rolldown will register included esm module's namespace object to the runtime.
  if ctx.config.dev_mode
    && module.idx != ctx.runtime_idx
    && matches!(ctx.facts.exports_kind(module.idx), ExportsKind::Esm)
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
fn sweep_side_effect_statements<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  module: &NormalModule,
) {
  let on_demand_side_effects = side_effects_included_on_demand_for(
    module,
    ctx.entry_module_idxs,
    ctx.facts.side_effects(module.idx),
    ctx.facts.exports_kind(module.idx),
  );
  ctx.stmt_infos[module.idx].iter_enumerated_without_namespace_stmt().for_each(
    |(stmt_info_id, stmt_info)| {
      // No need to handle the namespace statement specially, because it doesn't have side effects and will only be included if it is used.
      let bail_eval = module.meta.has_eval()
        && !stmt_info.declared_symbols.is_empty()
        && stmt_info_id.index() != 0;
      let has_side_effects = if module.meta.contains(EcmaViewMeta::SafelyTreeshakeCommonjs)
        && ctx.config.tree_shaking.commonjs
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
fn include_statements_without_treeshaking<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  module: &NormalModule,
) {
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
fn include_side_effectful_dependencies<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  module: &NormalModule,
) {
  let facts = ctx.facts;
  facts.dependencies(module.idx).for_each(|dependency_idx| {
    // Guard-hoist: skip already-included dependencies before paying the
    // `ctx.modules[idx]` match + `has_side_effects()` check. The authoritative
    // dedup is still `set_bit` inside `include_module`; this is a pure work-skip.
    if ctx.is_module_included_vec.has_bit(dependency_idx) {
      return;
    }
    match &ctx.modules[dependency_idx] {
      Module::Normal(importee) => {
        if !ctx.config.tree_shaking.enabled
          || ctx.facts.side_effects(importee.idx).has_side_effects()
        {
          ctx.pending.push(WorkItem::Module(importee.idx));
        }
      }
      Module::External(_) => {}
    }
  });
  tracing::trace!(
    "{}:\n module_meta dependencies: {:#?}",
    module.stable_id,
    facts.dependencies(module.idx).map(|idx| { ctx.modules[idx].id().to_string() }).collect_vec()
  );
}

pub(in crate::stages::link_stage) fn include_symbol<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  symbol_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  ctx.pending.push(WorkItem::Symbol(symbol_ref, include_reason));
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

fn handle_include_symbol<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
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
fn is_bypassed_inlined_constant<F: InclusionFacts>(
  ctx: &InclusionCoreContext<'_, F>,
  canonical_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) -> bool {
  if let Some(v) = ctx.facts.constant_export(&canonical_ref)
    && !include_reason.contains(SymbolIncludeReason::EntryExport)
    && (!ctx.config.inline_const_smart || v.safe_to_inline)
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
fn drain_body_demand_stmts<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  canonical_ref: SymbolRef,
) {
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
fn follow_cjs_namespace_alias<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  canonical_ref: &mut SymbolRef,
) {
  let canonical_ref_symbol = ctx.symbols.get(*canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    *canonical_ref = namespace_alias.namespace_ref;
    if let Some(idx) = ctx.facts.cjs_namespace_target(canonical_ref.owner, *canonical_ref) {
      // Include specific named export from CJS module.
      // Default import bailout is handled by check_cjs_bailout at call sites.
      // ```js
      // import {a} from './cjs.js'
      // console.log(a)
      // ```
      ctx.modules[idx].as_normal().inspect(|_| {
        let Some(export_symbol) =
          ctx.facts.resolved_export_symbol(idx, &namespace_alias.property_name)
        else {
          return;
        };
        if namespace_alias.property_name.as_str() != "default" {
          ctx.pending.push(WorkItem::Symbol(export_symbol, SymbolIncludeReason::Normal));
        }
      });
    }
  }
}

/// When the canonical is a module-namespace object, record *why* the namespace is included (the
/// finalizer emits it differently per reason). Returns whether this inclusion originates from a
/// simulated facade chunk, in which case the owner module itself must not be structurally
/// included.
fn note_namespace_inclusion_reason<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
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

/// Using any binding of a wrapped ESM module demands its `init_*` wrapper: the binding is only
/// initialized once the wrapper runs.
fn demand_esm_init_wrapper<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  canonical_ref: SymbolRef,
) {
  if let Some(wrapper_ref) = ctx
    .facts
    .esm_wrapper_ref(canonical_ref.owner)
    .filter(|wrapper_ref| *wrapper_ref != canonical_ref)
  {
    ctx.pending.push(WorkItem::Symbol(wrapper_ref, SymbolIncludeReason::Normal));
  }
}

/// Track which of a JSON module's top-level properties are referenced from *outside* its own
/// synthesized default export, so the finalizer knows which properties cannot be inlined.
fn note_json_self_reference<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
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
fn include_property_write_referencing_stmts<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  symbol_ref: SymbolRef,
) {
  if matches!(ctx.config.tree_shaking.property_write_side_effects, PropertyWriteSideEffects::False)
  {
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

pub(in crate::stages::link_stage) fn include_statement<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  module: &NormalModule,
  stmt_info_idx: StmtInfoIdx,
) {
  debug_assert!(ctx.pending.is_empty(), "engine queue must be empty between public entry points");
  ctx.pending.push(WorkItem::Statement(module.idx, stmt_info_idx));
  drain_work_items(ctx);
  debug_assert!(ctx.pending.is_empty(), "public entry points must drain the queue to empty");
}

fn handle_include_statement<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
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
        ctx.facts.member_expr_resolution(module.idx, member_expr_ref)
      }
    } {
      // Caveat: If we can get the member-expression resolution, it means this member expr
      // definitely contains a module namespace ref.
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
      std::iter::once(original_ref).chain(ctx.facts.normal_export_chain(original_ref)).for_each(
        |sym_ref| {
          enqueue_declaring_statements(ctx, sym_ref);
        },
      );
      push_symbol_and_check_cjs_bailout(ctx, *original_ref, include_kind);
    }
  });
}

/// FIXME: bailout for require() import for now
/// it is fine for now, since webpack did not support it either
/// ```js
/// const cjs = require('./cjs.js')
/// ```
fn scan_import_records_for_cjs_bailout<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
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
      if !matches!(ctx.facts.exports_kind(m.idx), ExportsKind::CommonJs)
        || import_record.kind == ImportKind::Import
      {
        return;
      }
      // Skip CJS bailout for dynamic imports that will be determined dead:
      // top-level pure (unused exports) importing a side-effect-free module.
      // The dynamic-entry fixpoint performs CJS bailout for retained live entries. Without this
      // check, a dead dynamic import's bailout would mark the module as included while its entry is
      // later removed, causing an empty-bits assertion in code splitting.
      if import_record.meta.contains(ImportRecordMeta::TopLevelPureDynamicImport)
        && !ctx.facts.side_effects(m.idx).has_side_effects()
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

pub(in crate::stages::link_stage) fn include_cjs_bailout_exports<
  F: InclusionFacts,
  I: IntoIterator<Item = ModuleIdx>,
>(
  ctx: &mut InclusionCoreContext<'_, F>,
  bailout_modules: I,
) {
  let facts = ctx.facts;
  for module_idx in bailout_modules {
    for symbol_ref in facts.commonjs_export_symbols(module_idx) {
      include_symbol_and_check_cjs_bailout(ctx, symbol_ref, SymbolIncludeReason::Normal);
    }
  }
}

pub(in crate::stages::link_stage) fn include_runtime_symbol<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
  runtime: &RuntimeModuleBrief,
  depended_runtime_helper: RuntimeHelper,
) {
  let runtime_module = &ctx.modules[runtime.id()].as_normal().expect("runtime should be normal");

  if depended_runtime_helper.is_empty() {
    // No runtime helpers needed, but if the runtime has side effects (e.g. from
    // a plugin transform), we still need to include it.
    if ctx.facts.side_effects(runtime_module.idx).has_side_effects() {
      include_module(ctx, runtime_module);
    }
    return;
  }

  for helper in depended_runtime_helper {
    let index = helper.bits().trailing_zeros() as usize;
    let name = RUNTIME_HELPER_NAMES[index];
    include_symbol(ctx, runtime.resolve_symbol(name), SymbolIncludeReason::Normal);
  }
}

/// Preserve re-exported interfaces for `preserveModules`.
///
/// Every module maps 1:1 to an output file whose `export { ... }` must mirror the source module's
/// interface. A re-export (`export { x } from './y'`) resolves to a *canonical* symbol owned by
/// `./y`, and consumers bind that canonical directly, bypassing this module's facade binding — so
/// the facade is tree-shaken out of this file's exports (issue #9122).
///
/// We re-mark a facade as used and include its re-export statement (so the cross-chunk import is
/// generated) only when the facade is actually consumed *through* this module — i.e. it appears as
/// an intermediate in the export chain of some used import. This is chain-granular: a re-export
/// nobody imports through this module stays tree-shaken, even when the same canonical is used via a
/// different module path; a genuinely-unused export likewise stays tree-shaken because no used
/// import reaches it. The synthetic runtime module is excluded.
///
/// Must run once, after the inclusion fixpoint has settled `used_symbol_refs`. It only includes
/// re-export statements that reference already-retained canonicals, so it introduces no new
/// reachable values and needs no further convergence.
pub(in crate::stages::link_stage) fn preserve_reexported_interfaces<F: InclusionFacts>(
  ctx: &mut InclusionCoreContext<'_, F>,
) {
  if !ctx.config.preserve_modules {
    return;
  }
  // Collect every intermediate re-export facade that lies on the export chain of a *used* imported
  // symbol — these are the facades consumed through their own module.
  let mut consumed_facades: FxHashSet<SymbolRef> = FxHashSet::default();
  let facts = ctx.facts;
  for (imported_as_ref, reexports) in facts.normal_export_chains() {
    if ctx.used_symbol_refs.contains(&imported_as_ref) {
      consumed_facades.extend(reexports.iter().copied());
    }
  }
  for symbol_ref in consumed_facades {
    let module_idx = symbol_ref.owner;
    if module_idx == ctx.runtime_idx || !ctx.is_module_included_vec.has_bit(module_idx) {
      continue;
    }
    let Module::Normal(module) = &ctx.modules[module_idx] else {
      continue;
    };
    let declaring_stmts = ctx.stmt_infos[module_idx].declared_stmts_by_symbol(&symbol_ref).to_vec();
    for stmt_info_id in declaring_stmts {
      include_statement(ctx, module, stmt_info_id);
    }
    include_symbol_and_check_cjs_bailout(ctx, symbol_ref, SymbolIncludeReason::EntryExport);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct IndependentFacts;

  impl InclusionModuleFacts for IndependentFacts {
    fn exports_kind(&self, _module_idx: ModuleIdx) -> ExportsKind {
      ExportsKind::None
    }

    fn side_effects(&self, _module_idx: ModuleIdx) -> DeterminedSideEffects {
      DeterminedSideEffects::Analyzed(false)
    }
  }

  impl InclusionFacts for IndependentFacts {
    fn cjs_namespace_target(
      &self,
      _importer_idx: ModuleIdx,
      _namespace_ref: SymbolRef,
    ) -> Option<ModuleIdx> {
      None
    }

    fn resolved_export_symbol(&self, _module_idx: ModuleIdx, _name: &str) -> Option<SymbolRef> {
      None
    }

    fn commonjs_export_symbols(
      &self,
      _module_idx: ModuleIdx,
    ) -> impl Iterator<Item = SymbolRef> + '_ {
      std::iter::empty()
    }

    fn included_commonjs_export_symbols(
      &self,
      _module_idx: ModuleIdx,
    ) -> impl Iterator<Item = SymbolRef> + '_ {
      std::iter::empty()
    }

    fn dependencies(&self, _module_idx: ModuleIdx) -> impl Iterator<Item = ModuleIdx> + '_ {
      std::iter::empty()
    }

    fn member_expr_resolution<'a>(
      &'a self,
      _module_idx: ModuleIdx,
      _member_expr_ref: &MemberExprRef,
    ) -> Option<&'a MemberExprRefResolution> {
      None
    }

    fn esm_wrapper_ref(&self, _module_idx: ModuleIdx) -> Option<SymbolRef> {
      None
    }

    fn constant_export(&self, _symbol_ref: &SymbolRef) -> Option<&ConstExportMeta> {
      None
    }

    fn normal_export_chain(&self, _symbol_ref: &SymbolRef) -> &[SymbolRef] {
      &[]
    }

    fn normal_export_chains(&self) -> impl Iterator<Item = (SymbolRef, &[SymbolRef])> + '_ {
      std::iter::empty()
    }
  }

  #[test]
  fn core_entrypoints_accept_an_independent_fact_provider() {
    let _ = include_symbol::<IndependentFacts>;
    let _ = include_module::<IndependentFacts>;
    let _ = include_statement::<IndependentFacts>;
    let _ = include_declaring_statements::<IndependentFacts>;
    let _ = include_symbol_and_check_cjs_bailout::<IndependentFacts>;
    let _ = include_runtime_symbol::<IndependentFacts>;
    let _ = preserve_reexported_interfaces::<IndependentFacts>;
    let _ = include_cjs_bailout_exports::<IndependentFacts, std::iter::Empty<ModuleIdx>>;
    let _ = compute_body_demand_keys_core::<IndependentFacts>;
  }

  #[test]
  fn core_source_has_no_legacy_carrier_dependency() {
    let source = include_str!("inclusion_core.rs");
    for forbidden in [
      concat!("Link", "Stage"),
      concat!("Linking", "Metadata"),
      concat!("Normalized", "BundlerOptions"),
      concat!("Shared", "Options"),
    ] {
      assert!(!source.contains(forbidden), "shared inclusion core contains `{forbidden}`");
    }
  }
}
