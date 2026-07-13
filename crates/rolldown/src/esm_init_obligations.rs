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
//! precomputed once by `compute_wrapped_esm_init_metadata` (post-convergence, stored as
//! `transitive_init_targets`), while Project must recompute them per fixpoint round from the
//! current plan — both through the shared excluded-hop router
//! [`collect_order_wrap_esm_init_targets`], so the routing itself cannot drift.
//!
//! Purpose contracts are deliberately *not* identical, and each divergence is encoded (and
//! justified) on [`ObligationPurpose`] rather than re-derived at call sites.

use rolldown_common::{
  ChunkIdx, ConstExportMeta, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta,
  IndexModules, InlineConstMode, Module, ModuleIdx, NormalModule, ResolvedImportRecord, Specifier,
  SymbolOrMemberExprRef, SymbolRef, SymbolRefDb, WrapKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  stages::generate_stage::order_wrap_state::{EsmInitOrigin, OrderWrapState},
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
) -> Vec<ModuleIdx> {
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

fn collect_esm_init_targets_for_record(
  ctx: &WrappedEsmInitTargetContext<'_>,
  rec_idx: ImportRecordIdx,
  symbol_is_used: &impl Fn(SymbolRef) -> bool,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  forwarding_module_owns_initialization: &impl Fn(ModuleIdx) -> bool,
  visited_forwarders: &mut FxHashSet<ModuleIdx>,
) -> Vec<ModuleIdx> {
  let mut targets = Vec::new();
  let record = &ctx.importer.import_records[rec_idx];
  let Some(importee_idx) = record.resolved_module else { return targets };
  let importee_meta = &ctx.metas[importee_idx];
  let route_through_transparent_wrapper =
    ctx.order_wrap_state.reexport_init_is_transparent(importee_idx)
      && !importee_meta.has_dynamic_exports
      && record_consumes_static_bindings(ctx.importer, record, rec_idx);

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
      targets.push(importee_idx);
      return targets;
    }
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

  targets
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
  targets: &mut Vec<ModuleIdx>,
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
  targets: &mut Vec<ModuleIdx>,
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
  targets: &mut Vec<ModuleIdx>,
  visited_symbols: &mut FxHashSet<SymbolRef>,
) {
  let canonical_ref = ctx.symbol_db.canonical_ref_resolving_namespace(symbol_ref);
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
    targets.push(canonical_ref.owner);
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
/// its own finalized statements — the full-delegation fast path. Its *included* import statements
/// do — the finalizer emits their `init_*()` calls at each statement's position — but a
/// static-import statement that tree-shaking excluded emits nothing there (a pure package barrel's
/// `export * from` hop whose bindings resolve through it is the canonical case). When every
/// static-import statement is included the forwarder owns every hop, so the caller can delegate
/// wholesale; when only some are, the caller delegates per obligation (see
/// [`forwarder_discharged_targets`]) rather than re-owning everything.
///
/// OPEN QUESTION (hypothesis, no repro — do not chase without one): this check consults only the
/// statement *inclusion* bits, while finalization additionally suppresses records marked "nested"
/// (`module_finalizers::mod` transform-or-remove and the `export *` path). If a directly consumed,
/// included hop could also be nested — and therefore emitted nowhere despite counting as
/// discharged here — the caller would wrongly delegate to a silent forwarder. No graph is known to
/// produce a nested *and* directly-consumed-included hop (nesting marks a record a wrapped ancestor
/// walks through, which owns the init instead), so this stays a documented invariant to revisit
/// only if a failing fixture appears.
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
      meta.stmt_info_included.has_bit(stmt_idx)
        || stmt_info
          .import_records
          .iter()
          .all(|rec_idx| module.import_records[*rec_idx].kind != ImportKind::Import)
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
) -> FxHashSet<ModuleIdx> {
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
  targets: &mut Vec<ModuleIdx>,
) {
  let mut stack = vec![root];
  while let Some(module_idx) = stack.pop() {
    let Module::Normal(importee) = &modules[module_idx] else { continue };
    let importee_linking_info = &metas[importee.idx];

    if !visited.insert(importee.idx) {
      continue;
    }

    // Only collect modules whose wrapper is declared (i.e. the module is included in the output)
    // and assigned to a chunk. Cross-chunk wrapper imports are registered after this pass.
    let transparent_retained_waypoint =
      retained_reexport_path.is_some() && order_state.reexport_init_is_transparent(importee.idx);
    if importee_linking_info.is_included
      && order_state.esm_init_included_in_live_chunk(
        importee_linking_info,
        importee.idx,
        chunk_graph,
      )
      && !transparent_retained_waypoint
    {
      targets.push(importee.idx);
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
      if retained_reexport_path.is_some_and(|path| !path.contains(&(importee.idx, rec_idx))) {
        continue;
      }
      if rec.kind == ImportKind::Import
        && let Some(sub_importee_idx) = rec.resolved_module
      {
        stack.push(sub_importee_idx);
      }
    }
  }
}
