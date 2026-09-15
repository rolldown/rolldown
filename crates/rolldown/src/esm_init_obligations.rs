//! The neutral home of every wrapped-ESM `init_*` **obligation** primitive — the one place that
//! answers "which wrapped modules must this importer's record initialize, and does this record
//! carry that obligation at all?".
//!
//! Three consumers enumerate the same module-level obligations for three purposes
//! ([`ObligationPurpose`]), and historically each carried its own copy of the record gating and
//! (before the emergent-cycle projection repair) its own route traversal, which let them drift
//! apart — the C-class under-projection holes were exactly such drift. Everything they share now
//! lives here:
//!
//! - **Emit** — the finalizer replaces each *included* static-import statement with the `init_*()`
//!   calls of the targets that record must initialize
//!   (`module_finalizers::transform_or_remove_import_export_stmt` and the `export *` path). It is
//!   AST-visitor-driven, so it consults [`record_is_init_obligation`] per record at the statement
//!   position (the statement is included by construction there) and resolves targets with
//!   [`collect_wrapped_esm_init_targets_for_import_record`], demanding the wrapper be *reachable in
//!   the emitting chunk*.
//! - **Register** — `compute_cross_chunk_links` registers the `init_*` wrapper symbols a chunk must
//!   import ahead of finalization. It drives [`for_each_init_obligation_record`] over the importer's
//!   included statements and resolves targets with the same collector, treating every wrapper as
//!   reachable (registration is what *makes* it reachable).
//! - **Project** — the on-demand emergent-cycle fixpoint (`order_analysis`) predicts the chunk
//!   edges a wrap plan's lowering will add, before anything is minted. It drives the same
//!   enumerator/collector against a probe [`OrderWrapState`], extended to the excluded re-export
//!   hops whose registration flows through the metadata pass rather than the included-record path.
//!
//! Excluded statements are the one structural asymmetry: for Emit and Register their targets are
//! precomputed once by `compute_wrapped_esm_init_metadata` (post-convergence, returned as
//! `Sealed<FinalEsmInitMetadata>`), while Project must recompute them per fixpoint round from the
//! current plan — both through the shared excluded-hop router
//! [`collect_order_wrap_esm_init_targets`], so the routing itself cannot drift.
//!
//! Purpose contracts are deliberately *not* identical, and each divergence is encoded (and
//! justified) on [`ObligationPurpose`] rather than re-derived at call sites.

use oxc_str::CompactStr;
use rolldown_common::{
  ChunkIdx, ConcatenateWrappedModuleKind, ConstExportMeta, ExportsKind, ImportKind,
  ImportRecordIdx, ImportRecordMeta, IndexModules, InlineConstMode, Module, ModuleIdx,
  NormalModule, ResolvedImportRecord, Specifier, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
  WrapKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  stages::generate_stage::order_wrap_state::{EsmInitOrigin, OrderCjsCarrierKey, OrderWrapState},
  type_alias::IndexStmtInfos,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

/// Why obligations are being enumerated. The variants select the *record-scope contract* — which
/// statements and records count as obligations — so a consumer states its contract once instead of
/// hand-rolling the gates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObligationPurpose {
  /// Finalizer emission at AST statement positions. Only *included* statements are visited (the
  /// finalizer's excluded statements consume the precomputed transitive metadata instead), and a
  /// nested re-export record emits nothing — a wrapped ancestor barrel walks through it and owns
  /// that init itself.
  Emit,
  /// Cross-chunk `init_*` symbol registration. Same contract as [`ObligationPurpose::Emit`]:
  /// included statements only, nested records skipped — registration and emission must stay in
  /// lockstep or a registered-but-never-emitted wrapper import (or vice versa) appears.
  Register,
  /// Emergent-cycle edge projection. Included statements *plus* excluded re-export hops (their
  /// real registration flows through the excluded-statement metadata, which does not exist yet at
  /// projection time), and nested records are *kept*: projection may over-approximate — an extra
  /// edge only ever wraps more, and wrapping more is always legal — but must never drop an edge
  /// source, so it declines the nested-ownership refinement Emit/Register apply.
  Project,
}

/// THE record gate: whether `rec` carries an init-forwarding obligation of the importer for this
/// purpose. All three consumers consult this one predicate (Emit per record at its included
/// statement position; Register/Project through [`for_each_init_obligation_record`]).
pub fn record_is_init_obligation(
  purpose: ObligationPurpose,
  order_state: &OrderWrapState,
  importer_idx: ModuleIdx,
  rec: &ResolvedImportRecord,
  rec_idx: ImportRecordIdx,
  stmt_is_included: bool,
) -> bool {
  if rec.kind != ImportKind::Import {
    return false;
  }
  if order_state.is_consumer_local_reexport_route(importer_idx)
    && rec.meta.intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly)
  {
    return false;
  }
  match purpose {
    ObligationPurpose::Emit | ObligationPurpose::Register => {
      stmt_is_included && !order_state.is_nested_reexport_record(importer_idx, rec_idx)
    }
    ObligationPurpose::Project => {
      stmt_is_included
        || rec.meta.intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly)
    }
  }
}

/// Drive [`record_is_init_obligation`] over every statement of one importer, calling `f` for each
/// obligation record. The statement-loop shape (including the namespace statement, whose record
/// list is empty) is shared by Register and Projection so their iteration order — and therefore
/// registration's insertion-ordered symbol set — cannot diverge.
pub fn for_each_init_obligation_record(
  purpose: ObligationPurpose,
  importer: &NormalModule,
  importer_meta: &LinkingMetadata,
  stmt_infos: &IndexStmtInfos,
  order_state: &OrderWrapState,
  mut f: impl FnMut(ImportRecordIdx),
) {
  for (stmt_info_idx, stmt_info) in stmt_infos[importer.idx].iter_enumerated() {
    let stmt_is_included = importer_meta.stmt_info_included.has_bit(stmt_info_idx);
    for &rec_idx in &stmt_info.import_records {
      if record_is_init_obligation(
        purpose,
        order_state,
        importer.idx,
        &importer.import_records[rec_idx],
        rec_idx,
        stmt_is_included,
      ) {
        f(rec_idx);
      }
    }
  }
}

/// Whether a re-export record **owns its forwarding hop**: an init-owning barrel forwards through
/// each of its re-export records unless the record is a nested walk-through interior a wrapped
/// ancestor's traversal already owns. This is the ownership half of the excluded-statement
/// forwarding predicate (`compute_wrapped_esm_init_metadata::order_wrap_record_forwards`) and the
/// same nested-record fact [`record_is_init_obligation`] consults for Emit/Register.
pub fn reexport_record_owns_hop(
  order_state: &OrderWrapState,
  importer_idx: ModuleIdx,
  rec_idx: ImportRecordIdx,
  is_reexport: bool,
) -> bool {
  is_reexport && !order_state.is_nested_reexport_record(importer_idx, rec_idx)
}

/// An ESM-wrapped module whose `init_*` an entry must run because the entry re-exports one of its
/// bindings (issue #10543).
pub struct EntryReexportedWrapperInit {
  pub owner: ModuleIdx,
  pub wrapper_ref: SymbolRef,
  /// A TLA-tainted wrapper renders as `await init_*()`. This can only surface in `esm` output:
  /// the scanner rejects top-level await under every other format
  /// (`AstScanner::handle_top_level_await`), so a TLA-tainted module never reaches emission
  /// there.
  pub tla_tainted: bool,
}

/// The record-less, off-strict obligation surface: the ESM-wrapped modules backing an entry's
/// re-exported bindings. Named re-exports resolve symbol-to-symbol, so when every forwarding
/// statement between the entry and such a module is tree-shaken, none of the record-scoped
/// consumers above ever sees the obligation — yet ESM semantics require the module to be
/// evaluated before the entry's bindings are read (issue #10543).
///
/// This is the single copy of the walk. Cross-chunk registration
/// (`collect_depended_symbols`'s entry branch) consumes it with `canonical_names: None` to import
/// every wrapper the entry may need; emission (the finalizer's entry body prelude and
/// `render_wrapped_entry_chunk`'s tail path) consumes it with the chunk's assigned names to call
/// exactly the reachable ones — so "everything emission calls, registration imported" is a fact
/// about the code rather than two enumerations kept in sync by hand.
///
/// Same-chunk owners are deliberately kept: an unwrapped barrel gets no excluded-statement init
/// metadata at all, so a fully tree-shaken same-chunk chain has no other call site. The one shape
/// where same-chunk owners are always covered — an entry wrapped by propagation, whose whole
/// static graph is wrapped with it — is filtered at its call site instead.
///
/// Results are in module execution order (dependencies before dependents). Both emission callers
/// gate on `!is_strict_execution_order_enabled()`: strict execution order routes entry
/// initialization through order-wrap lowering instead.
pub fn collect_entry_reexported_wrapper_inits(
  entry_id: ModuleIdx,
  entry_meta: &LinkingMetadata,
  metas: &LinkingMetadataVec,
  modules: &IndexModules,
  symbol_db: &SymbolRefDb,
  canonical_names: Option<&FxHashMap<SymbolRef, CompactStr>>,
) -> Vec<EntryReexportedWrapperInit> {
  // Filter before sorting: entries whose exports resolve to no wrapped module at all — the
  // overwhelmingly common case — should not pay for sorting their whole export map (a dep
  // optimizer barrel entry can have thousands of exports).
  let mut qualifying = entry_meta
    .resolved_exports
    .iter()
    .filter_map(|(name, resolved_export)| {
      if resolved_export.came_from_commonjs {
        return None;
      }
      let canonical_ref = symbol_db.canonical_ref_resolving_namespace(resolved_export.symbol_ref);
      if canonical_ref.owner == entry_id {
        return None;
      }
      let owner_meta = &metas[canonical_ref.owner];
      if !matches!(owner_meta.wrap_kind(), WrapKind::Esm)
        // An inner concatenated module's body runs via its group's shared wrapper; its own
        // `wrapper_ref` is not a callable declaration (mirrors the finalizer's emission skip).
        || matches!(
          owner_meta.concatenated_wrapped_module_kind,
          ConcatenateWrappedModuleKind::Inner
        )
      {
        return None;
      }
      let wrapper_ref = owner_meta.wrapper_ref?;
      if wrapper_ref == canonical_ref {
        return None;
      }
      let canonical_wrapper_ref = symbol_db.canonical_ref_for(wrapper_ref);
      // Emission may only call a wrapper the chunk declares or imports; anything else would
      // render as a dangling identifier. Registration passes `None`: it runs before chunk names
      // exist and is what makes a wrapper reachable in the first place.
      if let Some(canonical_names) = canonical_names
        && !canonical_names.contains_key(&canonical_wrapper_ref)
      {
        return None;
      }
      Some((name, canonical_ref.owner, wrapper_ref, canonical_wrapper_ref, owner_meta))
    })
    .collect::<Vec<_>>();
  // The export map iterates in hash order; sort the (rare) survivors by export name so the
  // first-export-wins dedup below is deterministic.
  qualifying.sort_unstable_by_key(|(name, ..)| *name);
  let mut seen_wrappers = FxHashSet::default();
  let mut inits = qualifying
    .into_iter()
    .filter_map(|(_, owner, wrapper_ref, canonical_wrapper_ref, owner_meta)| {
      seen_wrappers.insert(canonical_wrapper_ref).then_some(EntryReexportedWrapperInit {
        owner,
        wrapper_ref,
        tla_tainted: owner_meta.is_tla_or_contains_tla_dependency,
      })
    })
    .collect::<Vec<_>>();
  inits.sort_unstable_by_key(|init| modules[init.owner].exec_order());
  inits
}

pub struct WrappedEsmInitTargetContext<'a> {
  pub importer: &'a NormalModule,
  pub importer_meta: &'a LinkingMetadata,
  pub modules: &'a IndexModules,
  pub metas: &'a LinkingMetadataVec,
  pub stmt_infos: &'a IndexStmtInfos,
  pub symbol_db: &'a SymbolRefDb,
  pub constant_value_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
  pub inline_const_mode: Option<InlineConstMode>,
  pub order_wrap_state: &'a OrderWrapState,
  /// Strict-gates the forwarder discharge check so flag-off output stays byte-identical to main.
  pub strict_execution_order: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WrappedEsmInitTarget {
  Module(ModuleIdx),
  CjsCarrier(OrderCjsCarrierKey),
}

impl WrappedEsmInitTarget {
  pub fn owner(self) -> ModuleIdx {
    match self {
      Self::Module(module_idx) => module_idx,
      Self::CjsCarrier(key) => key.importer,
    }
  }
}

enum OrderInitTraversalItem {
  Module(ModuleIdx),
  Target(WrappedEsmInitTarget),
}

/// Resolve direct and forwarded ESM init targets for one static import record.
///
/// An eager (unwrapped) included same-chunk forwarder discharges the init of everything its own
/// finalized statements reach — its `init_*()` calls run at its earlier position in the shared
/// chunk. So a caller can delegate those targets to it. But a static-import statement tree-shaking
/// excluded (a pure barrel's `export * from` hop whose bindings resolve through it) emits nothing
/// there, so the forwarder does *not* discharge the targets that hop alone reaches, and the caller
/// must own them. The delegation is therefore **per obligation**: the caller resolves the wrapped
/// targets it consumes through the forwarder, then subtracts the ones the forwarder actually
/// discharges ([`forwarder_discharged_targets`]), owning only the difference — instead of the
/// module-wide all-or-nothing an earlier boolean forced (one unrelated excluded hop made the caller
/// re-own every binding). Full delegation is still an early-out when the forwarder discharges *all*
/// its hops or off-strict (flag-off parity with main).
pub fn collect_wrapped_esm_init_targets_for_import_record(
  ctx: &WrappedEsmInitTargetContext<'_>,
  rec_idx: ImportRecordIdx,
  symbol_is_used: impl Fn(SymbolRef) -> bool,
  wrapper_is_reachable: impl Fn(SymbolRef) -> bool,
  forwarding_module_owns_initialization: impl Fn(ModuleIdx) -> bool,
) -> Vec<WrappedEsmInitTarget> {
  let mut visited_forwarders = FxHashSet::default();
  collect_esm_init_targets_for_record(
    ctx,
    rec_idx,
    &symbol_is_used,
    &wrapper_is_reachable,
    &forwarding_module_owns_initialization,
    &mut visited_forwarders,
  )
}

/// Resolve the complete statically-known namespace of a consumer-local module. Normal named
/// consumers bypass the shared barrel wrapper and select only their bindings; a materialized
/// namespace must initialize every leaf and per-record CJS carrier instead of calling the
/// intentionally empty shared wrapper. Lowering caches this result for entry prologues and for
/// monolithic wrappers that expose the route's namespace.
pub fn collect_wrapped_esm_init_targets_for_module_namespace(
  ctx: &WrappedEsmInitTargetContext<'_>,
  wrapper_is_reachable: impl Fn(SymbolRef) -> bool,
) -> Vec<WrappedEsmInitTarget> {
  let mut targets = collect_eager_order_cjs_carriers_for_consumer_local_route(
    ctx,
    ctx.importer.idx,
    &wrapper_is_reachable,
  );
  let mut visited_symbols = FxHashSet::default();
  for export_name in ctx.importer_meta.sorted_and_non_ambiguous_resolved_exports.keys() {
    let resolved_export = &ctx.importer_meta.resolved_exports[export_name];
    add_wrapped_esm_init_target_for_symbol(
      ctx,
      resolved_export.symbol_ref,
      &wrapper_is_reachable,
      &mut targets,
      &mut visited_symbols,
    );
  }
  targets.sort_by_key(|target| consumer_local_target_order(ctx, ctx.importer.idx, *target));
  targets
}

pub fn collect_eager_order_cjs_carriers_for_consumer_local_route(
  ctx: &WrappedEsmInitTargetContext<'_>,
  module_idx: ModuleIdx,
  wrapper_is_reachable: impl Fn(SymbolRef) -> bool,
) -> Vec<WrappedEsmInitTarget> {
  let mut targets = Vec::new();
  let mut visited = FxHashSet::default();
  add_eager_order_cjs_carriers_for_consumer_local_route(
    ctx,
    module_idx,
    &wrapper_is_reachable,
    &mut targets,
    &mut visited,
  );
  targets
}

fn collect_esm_init_targets_for_record(
  ctx: &WrappedEsmInitTargetContext<'_>,
  rec_idx: ImportRecordIdx,
  symbol_is_used: &impl Fn(SymbolRef) -> bool,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  forwarding_module_owns_initialization: &impl Fn(ModuleIdx) -> bool,
  visited_forwarders: &mut FxHashSet<ModuleIdx>,
) -> Vec<WrappedEsmInitTarget> {
  let mut targets = Vec::new();
  let record = &ctx.importer.import_records[rec_idx];
  let Some(importee_idx) = record.resolved_module else { return targets };
  let importee_meta = &ctx.metas[importee_idx];
  let route_through_transparent_wrapper =
    ctx.order_wrap_state.reexport_init_is_transparent(importee_idx)
      && !importee_meta.has_dynamic_exports
      && (record_consumes_static_bindings(ctx.importer, record, rec_idx)
        || ctx.order_wrap_state.is_consumer_local_reexport_route(importee_idx));

  // An eager, unwrapped, included forwarder hosted in the importer's own chunk: it runs before the
  // importer in the shared chunk, so its own `init_*()` emission can be delegated to.
  let importee_is_eager_forwarder =
    ctx.order_wrap_state.esm_init_target(importee_idx, importee_meta).is_none()
      && matches!(importee_meta.wrap_kind(), WrapKind::None)
      && importee_meta.is_included
      && forwarding_module_owns_initialization(importee_idx);

  // Full delegation: off-strict keeps main's behavior (the forwarder owns everything); on-strict
  // this early-out fires only when the forwarder discharges *every* one of its hops, in which case
  // the per-obligation subtraction below would remove all targets anyway.
  if importee_is_eager_forwarder
    && (!ctx.strict_execution_order
      || eager_forwarder_discharges_own_hops(ctx, importee_idx, importee_meta))
  {
    return targets;
  }

  if wrapped_esm_target_is_reachable(
    importee_idx,
    importee_meta,
    ctx.order_wrap_state,
    wrapper_is_reachable,
  ) {
    if !route_through_transparent_wrapper {
      targets.push(WrappedEsmInitTarget::Module(importee_idx));
      return targets;
    }
  }

  if route_through_transparent_wrapper
    && ctx.order_wrap_state.is_consumer_local_reexport_route(importee_idx)
  {
    targets.extend(collect_eager_order_cjs_carriers_for_consumer_local_route(
      ctx,
      importee_idx,
      wrapper_is_reachable,
    ));
  }

  let mut visited_symbols = FxHashSet::default();
  if record.meta.contains(ImportRecordMeta::IsExportStar) {
    for export_name in importee_meta.sorted_and_non_ambiguous_resolved_exports.keys() {
      let resolved_export = &importee_meta.resolved_exports[export_name];
      add_wrapped_esm_init_target_for_symbol(
        ctx,
        resolved_export.symbol_ref,
        wrapper_is_reachable,
        &mut targets,
        &mut visited_symbols,
      );
    }
  } else {
    for (imported_as_ref, named_import) in
      ctx.importer.named_imports.iter().filter(|(_, item)| item.record_idx == rec_idx)
    {
      match &named_import.imported {
        Specifier::Star => {
          add_wrapped_esm_init_targets_for_namespace_consumer(
            ctx,
            *imported_as_ref,
            importee_meta,
            symbol_is_used,
            wrapper_is_reachable,
            &mut targets,
            &mut visited_symbols,
          );
        }
        Specifier::Literal(name) => {
          let symbol_ref = importee_meta
            .resolved_exports
            .get(name)
            .map_or(named_import.imported_as, |resolved_export| resolved_export.symbol_ref);
          // Liveness is importer-local. A named binding can itself hold a namespace object, so a
          // statically resolved `binding.member` read routes only that member even when the local
          // facade is absent from UsedSymbolRefs. Filtering by the canonical export would let a
          // different importer that consumes the same leaf resurrect this dead specifier.
          let binding_is_opaque = symbol_is_used(*imported_as_ref)
            || ctx.order_wrap_state.is_consumed_reexport_facade(*imported_as_ref)
            || add_wrapped_esm_init_targets_for_static_member_reads(
              ctx,
              *imported_as_ref,
              wrapper_is_reachable,
              &mut targets,
              &mut visited_symbols,
            );
          if binding_is_opaque {
            add_wrapped_esm_init_target_for_symbol(
              ctx,
              symbol_ref,
              wrapper_is_reachable,
              &mut targets,
              &mut visited_symbols,
            );
          }
        }
      }
    }
  }

  // Strict-mode per-obligation delegation to a *partial* forwarder: reaching here with an eager
  // forwarder means it does not discharge all its hops, so subtract exactly the targets it does
  // discharge and keep the rest.
  if importee_is_eager_forwarder {
    let discharged = forwarder_discharged_targets(
      ctx,
      importee_idx,
      symbol_is_used,
      wrapper_is_reachable,
      forwarding_module_owns_initialization,
      visited_forwarders,
    );
    targets.retain(|target| !discharged.contains(target));
  }

  if route_through_transparent_wrapper {
    targets.sort_by_key(|target| consumer_local_target_order(ctx, importee_idx, *target));
  }

  targets
}

fn consumer_local_target_order(
  ctx: &WrappedEsmInitTargetContext<'_>,
  forwarder_idx: ModuleIdx,
  target: WrappedEsmInitTarget,
) -> Vec<usize> {
  let mut visited = FxHashSet::default();
  consumer_local_target_order_path(ctx, forwarder_idx, target, &mut visited)
    .unwrap_or_else(|| vec![usize::MAX])
}

fn consumer_local_target_order_path(
  ctx: &WrappedEsmInitTargetContext<'_>,
  forwarder_idx: ModuleIdx,
  target: WrappedEsmInitTarget,
  visited: &mut FxHashSet<ModuleIdx>,
) -> Option<Vec<usize>> {
  if !visited.insert(forwarder_idx) {
    return None;
  }
  let Some(forwarder) = ctx.modules[forwarder_idx].as_normal() else {
    visited.remove(&forwarder_idx);
    return None;
  };
  if let WrappedEsmInitTarget::CjsCarrier(key) = target
    && key.importer == forwarder_idx
  {
    let position = forwarder
      .import_records
      .iter_enumerated()
      .position(|(rec_idx, _)| rec_idx == key.record)
      .unwrap_or(usize::MAX);
    visited.remove(&forwarder_idx);
    return Some(vec![position]);
  }

  let target_owner = target.owner();
  for (position, (rec_idx, rec)) in forwarder.import_records.iter_enumerated().enumerate() {
    if matches!(target, WrappedEsmInitTarget::Module(module_idx) if rec.resolved_module == Some(module_idx))
    {
      visited.remove(&forwarder_idx);
      return Some(vec![position]);
    }
    if let Some(importee_idx) = rec.resolved_module
      && (ctx.order_wrap_state.is_consumer_local_reexport_route(importee_idx)
        || ctx.order_wrap_state.reexport_init_is_transparent(importee_idx))
      && let Some(mut path) = consumer_local_target_order_path(ctx, importee_idx, target, visited)
    {
      path.insert(0, position);
      visited.remove(&forwarder_idx);
      return Some(path);
    }
    if forwarder.named_imports.iter().filter(|(_, import)| import.record_idx == rec_idx).any(
      |(imported_as_ref, _)| {
        ctx.symbol_db.canonical_ref_resolving_namespace(*imported_as_ref).owner == target_owner
      },
    ) {
      visited.remove(&forwarder_idx);
      return Some(vec![position]);
    }
    if rec.meta.contains(ImportRecordMeta::IsExportStar)
      && let Some(importee_idx) = rec.resolved_module
      && ctx.metas[importee_idx].resolved_exports.values().any(|resolved_export| {
        ctx.symbol_db.canonical_ref_resolving_namespace(resolved_export.symbol_ref).owner
          == target_owner
      })
    {
      visited.remove(&forwarder_idx);
      return Some(vec![position]);
    }
  }
  visited.remove(&forwarder_idx);
  None
}

fn add_eager_order_cjs_carriers_for_consumer_local_route(
  ctx: &WrappedEsmInitTargetContext<'_>,
  module_idx: ModuleIdx,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  targets: &mut Vec<WrappedEsmInitTarget>,
  visited: &mut FxHashSet<ModuleIdx>,
) {
  if !visited.insert(module_idx) {
    return;
  }
  let Some(module) = ctx.modules[module_idx].as_normal() else {
    return;
  };
  for (rec_idx, rec) in module.import_records.iter_enumerated() {
    let key = OrderCjsCarrierKey { importer: module_idx, record: rec_idx };
    if ctx.order_wrap_state.order_cjs_carrier(key).is_some_and(|carrier| carrier.eager) {
      add_order_cjs_carrier_target(ctx, key, wrapper_is_reachable, targets);
      continue;
    }
    if let Some(importee_idx) = rec.resolved_module
      && ctx.order_wrap_state.is_consumer_local_reexport_route(importee_idx)
      // Every forwarding hop must retain side effects. Without this per-hop gate, a deeper eager
      // carrier would leak through an outer `moduleSideEffects: false` barrel.
      && module.side_effects.has_side_effects()
      && ctx.modules[importee_idx].side_effects().has_side_effects()
    {
      add_eager_order_cjs_carriers_for_consumer_local_route(
        ctx,
        importee_idx,
        wrapper_is_reachable,
        targets,
        visited,
      );
    }
  }
}

/// Route a namespace import through only the members this importer actually reads. A statically
/// resolved `ns.x` reference retains `x`, not every export of the namespace; only an opaque use
/// such as passing `ns` as a value, computed access, re-export, or `eval` expands the full
/// non-ambiguous namespace. This is deliberately importer-local: module-global namespace or leaf
/// liveness can be caused by a different consumer and would reopen tree-shaking for this record.
/// See `internal-docs/code-splitting/design.md#tree-shaking-parity-across-strict-modes`.
fn add_wrapped_esm_init_targets_for_namespace_consumer(
  ctx: &WrappedEsmInitTargetContext<'_>,
  namespace_ref: SymbolRef,
  importee_meta: &LinkingMetadata,
  symbol_is_used: &impl Fn(SymbolRef) -> bool,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  targets: &mut Vec<WrappedEsmInitTarget>,
  visited_symbols: &mut FxHashSet<SymbolRef>,
) {
  let opaque_namespace_use = symbol_is_used(namespace_ref)
    || add_wrapped_esm_init_targets_for_static_member_reads(
      ctx,
      namespace_ref,
      wrapper_is_reachable,
      targets,
      visited_symbols,
    );

  if opaque_namespace_use {
    for export_name in importee_meta.sorted_and_non_ambiguous_resolved_exports.keys() {
      let resolved_export = &importee_meta.resolved_exports[export_name];
      add_wrapped_esm_init_target_for_symbol(
        ctx,
        resolved_export.symbol_ref,
        wrapper_is_reachable,
        targets,
        visited_symbols,
      );
    }
  }
}

/// Route statically resolved member reads of one local import facade and report whether any use is
/// opaque, in which case the caller must also initialize the imported binding as a whole.
fn add_wrapped_esm_init_targets_for_static_member_reads(
  ctx: &WrappedEsmInitTargetContext<'_>,
  local_ref: SymbolRef,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  targets: &mut Vec<WrappedEsmInitTarget>,
  visited_symbols: &mut FxHashSet<SymbolRef>,
) -> bool {
  let mut opaque_use = false;

  for (stmt_idx, stmt_info) in ctx.stmt_infos[ctx.importer.idx].iter_enumerated() {
    if !ctx.importer_meta.stmt_info_included.has_bit(stmt_idx) {
      continue;
    }
    for reference in &stmt_info.referenced_symbols {
      match reference {
        SymbolOrMemberExprRef::Symbol(symbol_ref) if *symbol_ref == local_ref => {
          opaque_use = true;
        }
        SymbolOrMemberExprRef::MemberExpr(member_expr) if member_expr.object_ref == local_ref => {
          match member_expr.resolution(&ctx.importer_meta.resolved_member_expr_refs) {
            Some(resolution) => {
              if let Some(symbol_ref) = resolution.resolved
                && !symbol_is_always_inlined(ctx, symbol_ref)
              {
                add_wrapped_esm_init_target_for_symbol(
                  ctx,
                  symbol_ref,
                  wrapper_is_reachable,
                  targets,
                  visited_symbols,
                );
              }
            }
            None => opaque_use = true,
          }
        }
        _ => {}
      }
    }
  }
  opaque_use
}

/// Match the inclusion pass's constant bypass for a resolved namespace member. The decision must
/// be per reference: consulting global symbol liveness alone lets another importer that needs the
/// same constant make this consumer initialize a module whose value was inlined here.
fn symbol_is_always_inlined(ctx: &WrappedEsmInitTargetContext<'_>, symbol_ref: SymbolRef) -> bool {
  let Some(mode) = ctx.inline_const_mode else {
    return false;
  };
  let canonical_ref = ctx.symbol_db.canonical_ref_for(symbol_ref);
  ctx.constant_value_map.get(&canonical_ref).is_some_and(|meta| {
    !meta.commonjs_export && (mode != InlineConstMode::Smart || meta.safe_to_inline)
  })
}

/// Whether this record has a statically resolvable binding consumer. A side-effect-only import has
/// no binding path to route and must keep calling a transparent wrapper directly. Dynamic-export
/// namespaces are filtered by the caller because their runtime re-export glue is not statically
/// replaceable with canonical leaf targets.
fn record_consumes_static_bindings(
  importer: &NormalModule,
  record: &ResolvedImportRecord,
  rec_idx: ImportRecordIdx,
) -> bool {
  record.meta.contains(ImportRecordMeta::IsExportStar)
    || importer.named_imports.values().any(|import| import.record_idx == rec_idx)
}

fn add_wrapped_esm_init_target_for_symbol(
  ctx: &WrappedEsmInitTargetContext<'_>,
  symbol_ref: SymbolRef,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  targets: &mut Vec<WrappedEsmInitTarget>,
  visited_symbols: &mut FxHashSet<SymbolRef>,
) {
  let canonical_ref = ctx.symbol_db.canonical_ref_resolving_namespace(symbol_ref);
  let mut carrier_keys = ctx.order_wrap_state.order_cjs_carriers_for_symbol(symbol_ref);
  if carrier_keys.is_empty() && canonical_ref != symbol_ref {
    carrier_keys = ctx.order_wrap_state.order_cjs_carriers_for_symbol(canonical_ref);
  }
  if !carrier_keys.is_empty() {
    for &key in carrier_keys {
      add_order_cjs_carrier_target(ctx, key, wrapper_is_reachable, targets);
    }
    return;
  }
  if !visited_symbols.insert(canonical_ref) {
    return;
  }
  let meta = &ctx.metas[canonical_ref.owner];
  let transparent_order_wrapper =
    ctx.order_wrap_state.reexport_init_is_transparent(canonical_ref.owner);
  if wrapped_esm_target_is_reachable(
    canonical_ref.owner,
    meta,
    ctx.order_wrap_state,
    wrapper_is_reachable,
  ) && !transparent_order_wrapper
  {
    targets.push(WrappedEsmInitTarget::Module(canonical_ref.owner));
    return;
  }

  let Some(module) = ctx.modules[canonical_ref.owner].as_normal() else {
    return;
  };
  let importer_is_order_wrapped = ctx
    .order_wrap_state
    .esm_init_target(ctx.importer.idx, ctx.importer_meta)
    .is_some_and(|target| matches!(target.origin, EsmInitOrigin::ExecutionOrder));
  if module.namespace_object_ref != canonical_ref
    || (meta.is_included && !transparent_order_wrapper)
    || (!transparent_order_wrapper && !importer_is_order_wrapped)
  {
    return;
  }

  for export_name in meta.sorted_and_non_ambiguous_resolved_exports.keys() {
    let resolved_export = &meta.resolved_exports[export_name];
    add_wrapped_esm_init_target_for_symbol(
      ctx,
      resolved_export.symbol_ref,
      wrapper_is_reachable,
      targets,
      visited_symbols,
    );
  }
}

/// Whether a static import record has importer-local binding demand even when tree shaking removes
/// the import declaration itself. Re-export flattening commonly excludes the declaration after its
/// binding resolves to a leaf; code-splitting placement must still route that consumer to the same
/// leaf/carrier that Emit/Register will use after wrapping.
pub fn import_record_has_live_binding_consumer(
  ctx: &WrappedEsmInitTargetContext<'_>,
  rec_idx: ImportRecordIdx,
  symbol_is_used: impl Fn(SymbolRef) -> bool,
) -> bool {
  ctx.importer.named_imports.iter().filter(|(_, import)| import.record_idx == rec_idx).any(
    |(local_ref, _)| {
      symbol_is_used(*local_ref)
        || ctx.order_wrap_state.is_consumed_reexport_facade(*local_ref)
        || ctx.stmt_infos[ctx.importer.idx].iter_enumerated().any(|(stmt_idx, stmt_info)| {
          ctx.importer_meta.stmt_info_included.has_bit(stmt_idx)
            && stmt_info.referenced_symbols.iter().any(|reference| match reference {
              SymbolOrMemberExprRef::Symbol(symbol_ref) => symbol_ref == local_ref,
              SymbolOrMemberExprRef::MemberExpr(member_expr) => {
                member_expr.object_ref == *local_ref
              }
            })
        })
    },
  )
}

fn add_order_cjs_carrier_target(
  ctx: &WrappedEsmInitTargetContext<'_>,
  key: OrderCjsCarrierKey,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  targets: &mut Vec<WrappedEsmInitTarget>,
) {
  let Some(carrier) = ctx.order_wrap_state.order_cjs_carrier(key) else {
    return;
  };
  let target = WrappedEsmInitTarget::CjsCarrier(key);
  if wrapper_is_reachable(carrier.wrapper_ref) && !targets.contains(&target) {
    targets.push(target);
  }
}

fn wrapped_esm_target_is_reachable(
  module_idx: ModuleIdx,
  meta: &LinkingMetadata,
  order_wrap_state: &OrderWrapState,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
) -> bool {
  order_wrap_state
    .esm_init_target(module_idx, meta)
    .is_some_and(|target| wrapper_is_reachable(target.wrapper_ref))
    && meta.is_included
    && !matches!(
      meta.concatenated_wrapped_module_kind,
      rolldown_common::ConcatenateWrappedModuleKind::Inner
    )
}

/// Whether an included, unwrapped forwarder discharges *all* its downstream initialization through
/// its own finalized statements — the full-delegation fast path. Its *included*, non-nested import
/// statements do — the finalizer emits their `init_*()` calls at each statement's position — but a
/// static-import statement that tree-shaking excluded emits nothing there (a pure package barrel's
/// `export * from` hop whose bindings resolve through it is the canonical case). When every
/// static-import record clears the finalizer's own record gate the forwarder owns every hop, so the
/// caller can delegate wholesale; when only some do, the caller delegates per obligation (see
/// [`forwarder_discharged_targets`]) rather than re-owning everything.
///
/// The record test mirrors Emit's gate ([`record_is_init_obligation`] for
/// [`ObligationPurpose::Emit`]) exactly: a static-import (`ImportKind::Import`) record discharges
/// only when its statement is *included* **and** the record is *not* a nested re-export
/// walk-through. Both conditions are precisely what the finalizer checks before emitting an
/// `init_*()` for a record, so full delegation can never count a record the finalizer actually
/// suppresses. A nested-but-included hop emits nothing at the forwarder — a wrapped ancestor's
/// traversal owns that init instead — yet the inclusion-bit-only predicate this replaced would have
/// treated it as discharged, delegating wholesale to a silent forwarder and dropping the init
/// altogether (the #10236 bug class). Any excluded or nested hop now fails this predicate and routes
/// the caller into the per-obligation partial-delegation path ([`forwarder_discharged_targets`]),
/// which enumerates through the same Emit gate and owns exactly the hops the forwarder leaves
/// silent. The direction is strictly conservative: tightening the gate can only keep a redundant
/// memoized `init_*` call, never drop a needed one.
fn eager_forwarder_discharges_own_hops(
  ctx: &WrappedEsmInitTargetContext<'_>,
  module_idx: ModuleIdx,
  meta: &LinkingMetadata,
) -> bool {
  let Some(module) = ctx.modules[module_idx].as_normal() else {
    return true;
  };
  ctx.stmt_infos[module_idx].iter_enumerated_without_namespace_stmt().all(
    |(stmt_idx, stmt_info)| {
      stmt_info.import_records.iter().all(|rec_idx| {
        module.import_records[*rec_idx].kind != ImportKind::Import
          || (meta.stmt_info_included.has_bit(stmt_idx)
            && !ctx.order_wrap_state.is_nested_reexport_record(module_idx, *rec_idx))
      })
    },
  )
}

/// The exact set of wrapped-ESM modules a *partial* eager forwarder discharges through its own
/// finalized statements: for each of the forwarder's **included**, non-nested static-import
/// records, the init targets that record's own emission reaches, resolved by the same collector the
/// forwarder itself runs when finalized (so this equals what the forwarder emits, never a superset —
/// subtracting it can only remove a redundant caller-side init, never a needed one).
///
/// A record tree-shaking excluded, or suppressed as a nested walk-through interior, emits nothing at
/// the forwarder and so discharges nothing (the caller must still own those). The forwarder is
/// hosted in the caller's own chunk (the delegation gate requires it), so the caller's
/// `wrapper_is_reachable` / same-chunk predicates apply unchanged to the forwarder's records.
/// `visited_forwarders` breaks same-chunk forwarder cycles by discharging nothing on re-entry
/// (under-approximating — a kept redundant init, never a dropped one).
fn forwarder_discharged_targets(
  ctx: &WrappedEsmInitTargetContext<'_>,
  forwarder_idx: ModuleIdx,
  symbol_is_used: &impl Fn(SymbolRef) -> bool,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  forwarding_module_owns_initialization: &impl Fn(ModuleIdx) -> bool,
  visited_forwarders: &mut FxHashSet<ModuleIdx>,
) -> FxHashSet<WrappedEsmInitTarget> {
  let mut discharged = FxHashSet::default();
  if !visited_forwarders.insert(forwarder_idx) {
    return discharged;
  }
  let Some(forwarder) = ctx.modules[forwarder_idx].as_normal() else {
    return discharged;
  };
  let forwarder_meta = &ctx.metas[forwarder_idx];
  let forwarder_ctx = WrappedEsmInitTargetContext {
    importer: forwarder,
    importer_meta: forwarder_meta,
    modules: ctx.modules,
    metas: ctx.metas,
    stmt_infos: ctx.stmt_infos,
    symbol_db: ctx.symbol_db,
    constant_value_map: ctx.constant_value_map,
    inline_const_mode: ctx.inline_const_mode,
    order_wrap_state: ctx.order_wrap_state,
    strict_execution_order: ctx.strict_execution_order,
  };
  // The forwarder's own emission contract is exactly Emit's: included statements, nested records
  // silent — enumerate its discharging records through the same purpose-gated enumerator.
  for_each_init_obligation_record(
    ObligationPurpose::Emit,
    forwarder,
    forwarder_meta,
    ctx.stmt_infos,
    ctx.order_wrap_state,
    |rec_idx| {
      discharged.extend(collect_esm_init_targets_for_record(
        &forwarder_ctx,
        rec_idx,
        symbol_is_used,
        wrapper_is_reachable,
        forwarding_module_owns_initialization,
        visited_forwarders,
      ));
    },
  );
  discharged
}

/// Follow excluded re-exports through barrels to included wrapped importees — the excluded-hop
/// router shared by the metadata pass (Emit/Register's precompute) and the fixpoint projector.
///
/// Called with `retained_reexport_path: None` on a *non-included* forwarder, it walks the
/// forwarder's every static import to the wrapped modules they reach — the excluded-hop routing the
/// real metadata pass performs, and the edge source the resolved-exports-only projection missed
/// (Hole 2). The real pass can pass `Some(path)` even through a non-included forwarder (retained
/// star paths are recorded pre-tree-shaking); the projector's `None` differs from that only at the
/// same-chunk prune below, and every retained-path target is a resolved export of the importer that
/// the projector already covers through its collector source — see
/// `project_excluded_forwarder_edges`.
#[expect(clippy::too_many_arguments)]
pub fn collect_order_wrap_esm_init_targets(
  modules: &IndexModules,
  metas: &LinkingMetadataVec,
  chunk_graph: &ChunkGraph,
  order_state: &OrderWrapState,
  importer_chunk_idx: ChunkIdx,
  root: ModuleIdx,
  retained_reexport_path: Option<&[(ModuleIdx, ImportRecordIdx)]>,
  visited: &mut FxHashSet<ModuleIdx>,
  targets: &mut Vec<WrappedEsmInitTarget>,
) {
  if retained_reexport_path.is_none() && order_state.is_consumer_local_reexport_route(root) {
    collect_live_eager_carriers_for_consumer_local_route(
      modules,
      chunk_graph,
      order_state,
      root,
      visited,
      targets,
    );
    return;
  }

  let retained_reexport_records = retained_reexport_path
    .map(|path| path.iter().copied().collect::<FxHashSet<(ModuleIdx, ImportRecordIdx)>>());
  let eager_retained_path_modules = retained_reexport_records
    .as_ref()
    .map(|path| collect_eager_retained_path_modules(modules, root, path));
  let mut stack = vec![OrderInitTraversalItem::Module(root)];
  while let Some(item) = stack.pop() {
    let module_idx = match item {
      OrderInitTraversalItem::Module(module_idx) => module_idx,
      OrderInitTraversalItem::Target(target) => {
        targets.push(target);
        continue;
      }
    };
    let Module::Normal(importee) = &modules[module_idx] else { continue };
    let importee_linking_info = &metas[importee.idx];

    if !visited.insert(importee.idx) {
      continue;
    }

    // Only collect modules whose wrapper is declared (i.e. the module is included in the output)
    // and assigned to a chunk. Cross-chunk wrapper imports are registered after this pass.
    let transparent_retained_waypoint = (retained_reexport_path.is_some()
      && order_state.reexport_init_is_transparent(importee.idx))
      || order_state.is_consumer_local_reexport_route(importee.idx);
    if importee_linking_info.is_included
      && order_state.esm_init_included_in_live_chunk(
        importee_linking_info,
        importee.idx,
        chunk_graph,
      )
      && !transparent_retained_waypoint
    {
      targets.push(WrappedEsmInitTarget::Module(importee.idx));
      continue;
    }

    if (retained_reexport_path.is_none()
      && importee_linking_info.is_included
      && chunk_graph.module_to_chunk[importee.idx] == Some(importer_chunk_idx))
      || !matches!(importee.exports_kind, ExportsKind::Esm | ExportsKind::None)
    {
      continue;
    }

    // Importee is a non-included barrel module — traverse its static imports to find included
    // wrapped importees transitively. Preserve recursive DFS order with an explicit LIFO stack:
    // pushing children in reverse keeps source-order visitation left-to-right.
    for (rec_idx, rec) in importee.import_records.iter_enumerated().rev() {
      if retained_reexport_records
        .as_ref()
        .is_some_and(|path| !path.contains(&(importee.idx, rec_idx)))
      {
        if eager_retained_path_modules
          .as_ref()
          .is_some_and(|modules| modules.contains(&importee.idx))
          && let Some(carrier) = order_state
            .order_cjs_carrier(OrderCjsCarrierKey { importer: importee.idx, record: rec_idx })
          && carrier.eager
          && order_state.order_cjs_carrier_included_in_live_chunk(
            OrderCjsCarrierKey { importer: importee.idx, record: rec_idx },
            chunk_graph,
          )
        {
          stack.push(OrderInitTraversalItem::Target(WrappedEsmInitTarget::CjsCarrier(
            OrderCjsCarrierKey { importer: importee.idx, record: rec_idx },
          )));
        }
        continue;
      }
      if rec.kind == ImportKind::Import
        && let Some(sub_importee_idx) = rec.resolved_module
      {
        let carrier_key = OrderCjsCarrierKey { importer: importee.idx, record: rec_idx };
        if order_state.has_order_cjs_carrier(carrier_key) {
          if order_state.order_cjs_carrier_included_in_live_chunk(carrier_key, chunk_graph) {
            stack
              .push(OrderInitTraversalItem::Target(WrappedEsmInitTarget::CjsCarrier(carrier_key)));
          }
          continue;
        }
        stack.push(OrderInitTraversalItem::Module(sub_importee_idx));
      }
    }
  }
}

/// Modules reached by a retained re-export path without crossing a `moduleSideEffects: false`
/// boundary. An eager carrier outside the selected binding path is an execution dependency only
/// while every forwarding hop retains side effects. Compute the path-wide permission up front so
/// a module reached by converging retained paths is allowed when any fully effectful path reaches
/// it, independent of DFS visitation order. Carriers explicitly selected by the retained path do
/// not consult this set: binding demand still has to initialize them across a pure boundary.
fn collect_eager_retained_path_modules(
  modules: &IndexModules,
  root: ModuleIdx,
  retained_records: &FxHashSet<(ModuleIdx, ImportRecordIdx)>,
) -> FxHashSet<ModuleIdx> {
  let mut eager_modules = FxHashSet::default();
  eager_modules.insert(root);
  let mut stack = vec![root];

  while let Some(module_idx) = stack.pop() {
    let Some(module) = modules[module_idx].as_normal() else {
      continue;
    };
    if !module.side_effects.has_side_effects() {
      continue;
    }
    for (rec_idx, rec) in module.import_records.iter_enumerated() {
      if rec.kind != ImportKind::Import || !retained_records.contains(&(module_idx, rec_idx)) {
        continue;
      }
      let Some(importee_idx) = rec.resolved_module else {
        continue;
      };
      if modules[importee_idx].side_effects().has_side_effects()
        && eager_modules.insert(importee_idx)
      {
        stack.push(importee_idx);
      }
    }
  }

  eager_modules
}

fn collect_live_eager_carriers_for_consumer_local_route(
  modules: &IndexModules,
  chunk_graph: &ChunkGraph,
  order_state: &OrderWrapState,
  module_idx: ModuleIdx,
  visited: &mut FxHashSet<ModuleIdx>,
  targets: &mut Vec<WrappedEsmInitTarget>,
) {
  if !visited.insert(module_idx) {
    return;
  }
  let Some(module) = modules[module_idx].as_normal() else {
    return;
  };
  for (rec_idx, rec) in module.import_records.iter_enumerated() {
    let key = OrderCjsCarrierKey { importer: module_idx, record: rec_idx };
    if let Some(carrier) = order_state.order_cjs_carrier(key) {
      if carrier.eager && order_state.order_cjs_carrier_included_in_live_chunk(key, chunk_graph) {
        targets.push(WrappedEsmInitTarget::CjsCarrier(key));
      }
      continue;
    }
    if let Some(importee_idx) = rec.resolved_module
      && order_state.is_consumer_local_reexport_route(importee_idx)
      // Keep the precomputed excluded-hop path under the same per-hop
      // `moduleSideEffects` boundary as importer-local record routing.
      && module.side_effects.has_side_effects()
      && modules[importee_idx].side_effects().has_side_effects()
    {
      collect_live_eager_carriers_for_consumer_local_route(
        modules,
        chunk_graph,
        order_state,
        importee_idx,
        visited,
        targets,
      );
    }
  }
}
