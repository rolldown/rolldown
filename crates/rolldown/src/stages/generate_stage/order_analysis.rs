use std::sync::LazyLock;

use itertools::Itertools;
use oxc_index::{IndexVec, index_vec};
use petgraph::prelude::DiGraphMap;
use rolldown_common::{
  ChunkIdx, ConcatenateWrappedModuleKind, EcmaViewMeta, ExportsKind, ImportKind, ImportRecordIdx,
  ImportRecordMeta, Module, ModuleIdx, NormalModule, SymbolOrMemberExprRef, SymbolRef,
  UsedSymbolRefsBuilder, WrapKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;
use crate::esm_init_obligations::{
  ObligationPurpose, WrappedEsmInitTargetContext, collect_order_wrap_esm_init_targets,
  collect_wrapped_esm_init_targets_for_import_record, for_each_init_obligation_record,
};

use super::GenerateStage;
use super::order_wrap_state::EsmInitOrigin;

/// `ROLLDOWN_ORDER_DEBUG=1` turns on a stderr trace of the on-demand emergent-cycle fixpoint:
/// the one-shot plan size, per-round emergent-cyclic-SCC shape and at-risk growth, and the final
/// wrap delta over the one-shot plan. Off by default — the flag is read once into this
/// `LazyLock`, so a disabled trace is a single relaxed atomic load outside the fixpoint's hot
/// loop and costs nothing in normal builds. It makes otherwise-unverifiable claims (e.g.
/// "vue-vben-admin: +141 wraps in 2 iterations") reproducible from build artifacts.
static ORDER_DEBUG: LazyLock<bool> = LazyLock::new(|| {
  std::env::var_os("ROLLDOWN_ORDER_DEBUG").is_some_and(|value| value != "0" && !value.is_empty())
});

/// Emit one `ROLLDOWN_ORDER_DEBUG` trace line. The message closure runs only when the flag is on,
/// so building the (allocating) string is skipped entirely in normal builds.
fn order_debug_trace(message: impl FnOnce() -> String) {
  if *ORDER_DEBUG {
    #[expect(clippy::print_stderr, reason = "opt-in ROLLDOWN_ORDER_DEBUG diagnostic trace")]
    {
      eprintln!("{}", message());
    }
  }
}

#[derive(Debug)]
pub(super) struct OrderAnalysis {
  pub(super) plan: OrderWrapPlan,
  pub(super) import_edges: IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
  /// Whether this analysis was produced by the selective on-demand mode (as opposed to wrap-all).
  /// Lowering reads it to decide whether entry-facade splitting is conditional (on-demand) or
  /// unconditional (wrap-all) instead of re-reading the `experimental.onDemandWrapping` option, so
  /// the wrapping policy lives in exactly one place — the analysis that already branched on it.
  pub(super) on_demand: bool,
}

#[derive(Debug, Default)]
pub(super) struct OrderWrapPlan {
  modules: FxHashSet<ModuleIdx>,
}

impl OrderWrapPlan {
  fn insert(&mut self, module_idx: ModuleIdx) -> bool {
    self.modules.insert(module_idx)
  }

  pub(super) fn contains(&self, module_idx: &ModuleIdx) -> bool {
    self.modules.contains(module_idx)
  }

  pub(super) fn is_empty(&self) -> bool {
    self.modules.is_empty()
  }

  pub(super) fn len(&self) -> usize {
    self.modules.len()
  }

  pub(super) fn modules(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.modules.iter().copied()
  }
}

#[derive(Debug)]
struct RootOrderAnalysis {
  root: ModuleIdx,
  expected_order: Vec<ModuleIdx>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
  Unvisited,
  Visiting,
  Done,
}

struct ActualOrderTraversal {
  chunk_states: IndexVec<ChunkIdx, VisitState>,
  module_states: IndexVec<ModuleIdx, VisitState>,
  order: Vec<ModuleIdx>,
  trigger_hosts: FxHashMap<ModuleIdx, ModuleIdx>,
}

impl GenerateStage<'_> {
  pub(super) fn analyze_execution_order(
    &mut self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &UsedSymbolRefsBuilder,
  ) -> Option<OrderAnalysis> {
    if !self.options.is_strict_execution_order_enabled() {
      return None;
    }

    // Wrap-all is the default strict mode: every eligible module defers, so the eager phase is
    // inert and no evaluation-order prediction is needed. The on-demand analysis below is the
    // opt-in selective mode behind `experimental.onDemandWrapping`.
    if !self.options.experimental.is_on_demand_wrapping_enabled() {
      return Some(self.wrap_all_order_analysis(chunk_graph));
    }

    let import_edges = self.predicted_static_import_edges(chunk_graph, used_symbol_refs);
    let chunk_cycles = ChunkCycles::from_import_edges(&import_edges);
    let mut all_at_risk = FxHashSet::default();
    let mut roots = Vec::new();

    for &root in self.link_output.entries.keys() {
      if !self.link_output.metas[root].is_included {
        continue;
      }
      let Some(&root_chunk) = chunk_graph.entry_module_to_entry_chunk.get(&root) else {
        continue;
      };

      let expected_order = self.expected_order_for_root(root);
      let (actual_order, trigger_hosts) =
        self.actual_order_for_root(root, root_chunk, chunk_graph, &import_edges);
      let mut at_risk = self.at_risk_modules(&expected_order, &actual_order, &trigger_hosts);
      // Lowering can change the entry edge into a static chunk cycle, so wrap every eligible
      // source module under a root that reaches one.
      if chunk_cycles.reachable_from(root_chunk, &import_edges) {
        at_risk
          .extend(expected_order.iter().copied().filter(|idx| self.is_order_wrap_eligible(*idx)));
      }
      all_at_risk.extend(at_risk.iter().copied());
      roots.push(RootOrderAnalysis { root, expected_order });
    }

    // The plan is computed against the *predicted* (pre-lowering) chunk edges, which are acyclic
    // for the apps that hit this bug. But applying the plan makes the lowering add its own
    // cross-chunk imports — `init_*` wrapper imports and value imports of newly-wrapped modules —
    // which can close chunk cycles the one-shot analysis never saw. An eager module hosted in such
    // an emergent-cycle chunk runs its record-position `init_*()` during the cycle's evaluation and
    // reads a sibling chunk that has not been assigned yet (vue-vben-admin: `qe is not a function`,
    // where `qe = __commonJSMin(dayjs/plugin/timezone)` lives in a cycle-sibling chunk). Wrap-all is
    // immune because it defers every module body, so a cyclic chunk holds only hoisted declarations;
    // on-demand must close the loop: recompute the chunk edges *including* the lowering's added
    // imports for the current plan, mark every eligible module hosted in a resulting cyclic chunk
    // at-risk, and repeat. That makes the cyclic chunks wrap-all-equivalent (no eager body runs
    // mid-cycle), which is the standing correctness proof; the extra wrapping is bounded to
    // emergent-cycle members, the at-risk set only grows, and it is finite — so this converges.
    //
    // Both inputs below are plan-independent, so they are computed once for every plan rebuild:
    // the roots never change across rounds, and the reverse index inverts the static module graph,
    // not anything the plan mints.
    let source_reachable = self.source_reachable_modules(&roots);
    let reverse_static_imports =
      reverse_static_import_index(&self.link_output.module_table.modules);
    let mut plan = self.build_order_wrap_plan(
      all_at_risk.clone(),
      &roots,
      chunk_graph,
      &chunk_cycles,
      &source_reachable,
      &reverse_static_imports,
    );
    let one_shot_planned = plan.len();
    order_debug_trace(|| {
      format!(
        "[order] one-shot plan: {one_shot_planned} modules ({} pre-lowering chunk cycles)",
        chunk_cycles.sccs.len(),
      )
    });
    // The at-risk set is monotone and finite; this bound only guards against a logic error.
    let iteration_cap = self.link_output.module_table.modules.len() + 1;
    let mut iterations = 0usize;
    loop {
      // Project the chunk edges the current plan's lowering will add — the `init_*` forwarding
      // imports of wrapped modules — on top of the pre-lowering baseline, then find the chunk
      // cycles those emergent edges close.
      let post_edges = self.post_lowering_import_edges(
        chunk_graph,
        &plan,
        &import_edges,
        used_symbol_refs,
        &reverse_static_imports,
      );
      let post_cycles = ChunkCycles::from_import_edges(&post_edges);
      // Mark every eligible module hosted in an emergent cyclic chunk at-risk. Wrapping them all
      // (not only the order-sensitive ones) removes every eager body from those chunks, matching the
      // wrap-all shape that is provably safe under cyclic evaluation. `all_at_risk` only grows.
      let mut added = 0usize;
      for scc in &post_cycles.sccs {
        for &chunk_idx in scc {
          for &module_idx in &chunk_graph.chunk_table[chunk_idx].modules {
            if self.is_order_wrap_eligible(module_idx) && all_at_risk.insert(module_idx) {
              added += 1;
            }
          }
        }
      }
      iterations += 1;
      order_debug_trace(|| {
        format!(
          "[order] fixpoint round {iterations}: {} emergent cyclic chunk SCC(s), +{added} at-risk (total {})",
          post_cycles.sccs.len(),
          all_at_risk.len(),
        )
      });
      if added == 0 {
        break;
      }
      plan = self.build_order_wrap_plan(
        all_at_risk.clone(),
        &roots,
        chunk_graph,
        &chunk_cycles,
        &source_reachable,
        &reverse_static_imports,
      );
      assert!(iterations < iteration_cap, "order-wrap emergent-cycle fixpoint did not converge");
    }
    order_debug_trace(|| {
      format!(
        "[order] fixpoint converged in {iterations} iteration(s): {} modules planned (+{} over the one-shot plan)",
        plan.len(),
        plan.len() - one_shot_planned,
      )
    });
    tracing::debug!(
      target: "order_analysis",
      iterations,
      planned_modules = plan.len(),
      "emergent-cycle fixpoint converged"
    );
    Some(OrderAnalysis { plan, import_edges, on_demand: true })
  }

  /// Project the chunk-level static import edges the lowering of `plan` will produce, as the
  /// pre-lowering `baseline` edges plus the cross-chunk `init_*` forwarding edges wrapping adds.
  /// Applying a wrap plan makes the linker register three distinct kinds of `init_*` dependency
  /// (`add_module_esm_init_depended_symbols`), and this projection reproduces all three from a
  /// discovery-only probe state — carrying the same order wrappers, nested-record set, and per-record
  /// overlays the real lowering mints (see `probe_order_state`) — so the fixpoint sees exactly the
  /// emergent cycles the real link pass will close. Every projected edge source, and every
  /// deliberately-omitted one, is enumerated here.
  ///
  /// PROJECTED:
  /// - **Retained re-export overlays** ([`Self::project_reexport_overlay_edges`]) — an importer that
  ///   re-exports (or has an active execution-dependency import of) an order-wrapped module
  ///   references that module's wrapper from its own chunk, *even when the importer is eager and
  ///   owns no `init_*`*. Mirrors the `OrderImportOverlay` `lower_order_state` mints and
  ///   `add_order_import_overlay_depended_symbols` registers with no init-owner gate. The old
  ///   projection walked only importers owning an `init_*`, so it missed every eager forwarder — see
  ///   the `emergent_cycle_eager_reexport_overlay` fixture.
  /// - **Included + retained excluded re-export forwarding** ([`Self::project_collector_edges`]) — a
  ///   wrapped importer's `init_*` calls the `init_*` of every wrapped module its included static
  ///   imports and retained excluded re-export hops reach, resolved by the finalizer's own
  ///   `collect_wrapped_esm_init_targets_for_import_record` (mirrors
  ///   `add_included_import_esm_init_depended_symbols` and the resolved-exports registration). This
  ///   is the drift-free core the original projection already performed.
  /// - **Non-included forwarder hops** ([`Self::project_excluded_forwarder_edges`]) — a wrapped
  ///   importer's re-export of a *non-included* forwarder forwards to every wrapped module the
  ///   forwarder's static imports reach, walking the forwarder itself (not just its resolved
  ///   exports) via the shared `collect_order_wrap_esm_init_targets`. Mirrors the excluded-statement
  ///   metadata routing `add_transitive_esm_init_depended_symbols` registers — invisible to the
  ///   resolved-exports-only projection (see the `emergent_cycle_excluded_forwarder_import` fixture).
  ///
  /// DELIBERATELY OMITTED:
  /// - **Interop `WrapKind::Esm` overlay edges and consumption-gated hops** — the overlay projection
  ///   admits only order-wrapped (`EsmInitOrigin::ExecutionOrder`) direct targets whose record is not
  ///   a nested re-export. An interop target's wrapper edge already exists in the flag-off baseline,
  ///   and a nested hop's init is owned by a wrapped ancestor and tree-shaken away for an eager
  ///   forwarder (its `init_*` import is DCE'd). Projecting either would fabricate a cycle and
  ///   over-wrap a tree-shaking-equivalent graph (the `retained_star_renamed_cycle` shape).
  /// - **Entry-facade transitive init imports** — an entry facade holds zero modules, so it has zero
  ///   internal static indegree and can never sit inside a static chunk SCC; only dynamic edges route
  ///   through it (asserted in `compute_cross_chunk_links`). So no facade edge is constructible.
  ///
  /// Unioning the projected edges with the baseline value/side-effect edges still over-approximates
  /// the real post-lowering topology — it omits the wrapping's own liveness suppression — which is
  /// sound: extra edges only ever wrap more, and wrapping more is always legal (wrap-all wraps
  /// everything and is the standing correctness proof).
  ///
  /// This is a pure projection keyed on `ModuleIdx`: it neither mints wrapper symbols nor mutates
  /// symbol chunk ownership. It reuses the finalizer's and the metadata pass's own target resolvers,
  /// fed a probe order state that marks the planned modules wrapped, so it stays in lockstep with
  /// what the linker registers and the finalizer emits.
  fn post_lowering_import_edges(
    &self,
    chunk_graph: &ChunkGraph,
    plan: &OrderWrapPlan,
    baseline: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
    used_symbol_refs: &UsedSymbolRefsBuilder,
    reverse_static_imports: &IndexVec<ModuleIdx, Vec<ModuleIdx>>,
  ) -> IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> {
    let mut edges = baseline.clone();
    let probe_state =
      self.probe_order_state(chunk_graph, plan, used_symbol_refs, reverse_static_imports);

    for module in self.link_output.module_table.modules.iter().filter_map(Module::as_normal) {
      let importer_idx = module.idx;
      let meta = &self.link_output.metas[importer_idx];
      let Some(importer_chunk) = chunk_graph.module_to_chunk[importer_idx] else {
        continue;
      };

      let mut targets = Vec::new();

      // Retained re-export overlays apply to *every* importer, eager or wrapped.
      self.project_reexport_overlay_edges(&probe_state, module, &mut targets);

      // Init forwarding through included imports and excluded re-export hops only applies to
      // importers that carry their own ESM `init_*` (an order-wrapped plan member or an interop
      // `WrapKind::Esm` module).
      if probe_state.esm_init_target(importer_idx, meta).is_some() {
        self.project_collector_edges(
          chunk_graph,
          &probe_state,
          module,
          meta,
          importer_chunk,
          &mut targets,
        );
        self.project_excluded_forwarder_edges(
          chunk_graph,
          &probe_state,
          module,
          importer_chunk,
          &mut targets,
        );
      }

      for target_idx in targets {
        if let Some(target_chunk) = chunk_graph.module_to_chunk[target_idx]
          && target_chunk != importer_chunk
          && chunk_graph.module_is_in_live_chunk(target_idx)
        {
          edges[importer_chunk].insert(target_chunk);
        }
      }
    }

    edges
  }

  /// Retained re-export overlay projection — mirrors `add_order_import_overlay_depended_symbols`,
  /// which registers an importer's `OrderImportOverlay` referenced symbols (the direct target's
  /// `init_*` wrapper, plus namespaces) from the importer's chunk with no gate on whether the
  /// importer owns an `init_*`. The probe carries the same overlays `lower_order_state` mints
  /// (populated in `probe_order_state`), so this surfaces an *eager* forwarder's cross-chunk hop
  /// exactly as the linker will register it — the edge source the wrapped-only projection missed
  /// (Hole 1).
  ///
  /// A wrapper-referencing overlay (non-empty `referenced_symbols`) always names its own record's
  /// direct target, so the projected chunk edge is `importer -> record.resolved_module`. Reading
  /// the record's resolved module rather than the referenced symbol's owner is deliberate: the
  /// probe stands each not-yet-minted wrapper in as the module's namespace ref, and resolving that
  /// placeholder through the export chain can canonicalize to a *different* owner (e.g. a renamed
  /// star re-export), which would fabricate a phantom edge. A `transitive_reexport` overlay carries
  /// no referenced symbols — its hop is registered through the metadata path, not a direct edge —
  /// so it is skipped here and covered by the collector / non-included-forwarder projections.
  fn project_reexport_overlay_edges(
    &self,
    probe_state: &super::order_wrap_state::OrderWrapState,
    module: &NormalModule,
    targets: &mut Vec<ModuleIdx>,
  ) {
    for (key, overlay) in probe_state.import_overlays_for_importer(module.idx) {
      // A `transitive_reexport` overlay (no referenced symbols) routes its init through the
      // metadata path, not a direct edge, so it is covered by the collector / non-included-forwarder
      // projections instead.
      if overlay.referenced_symbols.is_empty() {
        continue;
      }
      // A nested re-export record is one a wrapped ancestor barrel walks *through* to own the init
      // itself, so the record's own module emits nothing for it — for an eager forwarder that hop's
      // `init_*` import is DCE'd, and projecting it would fabricate a cycle and over-wrap a
      // tree-shaking-equivalent graph (the `retained_star_renamed_cycle` shape, where a dead renamed
      // re-export is nested under a consuming star re-export).
      if probe_state.is_nested_reexport_record(module.idx, key.record) {
        // Skipping cannot drop a real edge; the two ways a *nested* record can carry a
        // wrapper-referencing overlay are both covered elsewhere:
        // - The record also heads a retained star path (`root_paths` takes precedence over
        //   `nested_records` in `retained_order_reexport_path`, and paths are recorded
        //   pre-tree-shaking), so the overlay carries a non-empty `retained_reexport_path`. The
        //   finalizer skips record-position emission for such overlays and the owning wrapped
        //   ancestor's retained-path traversal performs the init instead — projected from the
        //   ancestor by the collector source. (`retained_star_renamed_cycle` exercises this.)
        // - With an *empty* retained path, minting the overlay requires an execution dependency on
        //   a planned target, which `build_order_wrap_plan`'s closure
        //   (`statically_imports_wrapped_member`) turns into wrapping this importer itself before
        //   any probe is built — and a wrapped (or interop `WrapKind::Esm`) importer's records are
        //   never nested (`module_owns_reexport_init`), so for a live importer this combination is
        //   unreachable. Assert it so the unproven corner (a concatenated-inner importer — a
        //   dormant feature on this branch) trips loudly instead of silently dropping an edge.
        debug_assert!(
          !overlay.retained_reexport_path.is_empty()
            || !self.link_output.metas[module.idx].is_included
            || !matches!(
              self.link_output.metas[module.idx].concatenated_wrapped_module_kind,
              ConcatenateWrappedModuleKind::None
            ),
          "an included importer's nested re-export record carries a wrapper-referencing order \
           overlay with no retained path; the plan closure should have wrapped the importer first",
        );
        continue;
      }
      let Some(target_idx) = module.import_records[key.record].resolved_module else {
        continue;
      };
      // Only an order-wrapped direct target is a *new* forwarding edge this plan adds. An interop
      // `WrapKind::Esm` target already carries its wrapper in flag-off output, so its overlay edge
      // is in the baseline rather than an emergent one.
      if probe_state
        .esm_init_target(target_idx, &self.link_output.metas[target_idx])
        .is_some_and(|target| matches!(target.origin, EsmInitOrigin::ExecutionOrder))
      {
        targets.push(target_idx);
      }
    }
  }

  /// Included-import and retained excluded-re-export projection — mirrors both
  /// `add_included_import_esm_init_depended_symbols` and the resolved-exports registration of a
  /// wrapped importer's retained re-export hops: for each included statement, and each excluded
  /// re-export hop, resolve targets with the finalizer's own
  /// `collect_wrapped_esm_init_targets_for_import_record`, treating every wrapper as reachable since
  /// we are discovering which chunk edges *would* be created. This is the drift-free core the
  /// original projection already performed; the two holes are closed by the overlay and
  /// non-included-forwarder projections beside it.
  fn project_collector_edges(
    &self,
    chunk_graph: &ChunkGraph,
    probe_state: &super::order_wrap_state::OrderWrapState,
    module: &NormalModule,
    meta: &crate::types::linking_metadata::LinkingMetadata,
    importer_chunk: ChunkIdx,
    targets: &mut Vec<ModuleIdx>,
  ) {
    let ctx = WrappedEsmInitTargetContext {
      importer: module,
      importer_meta: meta,
      modules: &self.link_output.module_table.modules,
      metas: &self.link_output.metas,
      stmt_infos: &self.link_output.stmt_infos,
      symbol_db: &self.link_output.symbol_db,
      order_wrap_state: probe_state,
      strict_execution_order: true,
    };
    // Enumerate through the shared purpose-gated enumerator (Project contract: included
    // statements plus excluded re-export hops — an included statement's targets are emitted at its
    // own position, an *excluded* statement still forwards when it is a re-export hop the wrapped
    // importer owns, and non-re-export excluded records forward nothing).
    for_each_init_obligation_record(
      ObligationPurpose::Project,
      module,
      meta,
      &self.link_output.stmt_infos,
      probe_state,
      |rec_idx| {
        targets.extend(collect_wrapped_esm_init_targets_for_import_record(
          &ctx,
          rec_idx,
          |_| true,
          |forwarding_module_idx| {
            chunk_graph.module_to_chunk[forwarding_module_idx] == Some(importer_chunk)
          },
        ));
      },
    );
  }

  /// Excluded non-included-forwarder projection — a wrapped importer's re-export of a *non-included*
  /// forwarder forwards init to every wrapped module the forwarder's static imports reach, walking
  /// the forwarder itself rather than only its resolved exports. This mirrors the excluded-statement
  /// metadata routing (`transitive_esm_init_targets` → `collect_order_wrap_esm_init_targets`) that
  /// `add_transitive_esm_init_depended_symbols` registers. This is the edge source the
  /// resolved-exports-only projection missed (Hole 2).
  ///
  /// The walk passes `retained_reexport_path: None` while the real metadata pass can carry `Some`
  /// even through a non-included forwarder — retained star paths are recorded pre-tree-shaking
  /// (`record_star_reexport_path`), so a path can structurally route through a forwarder that later
  /// loses inclusion. The two calls then differ only at the same-chunk prune inside
  /// `collect_order_wrap_esm_init_targets`: `None` prunes a same-chunk included waypoint where
  /// `Some(path)` walks through it. That divergence never loses a real cross-chunk edge: every
  /// target reachable across a retained re-export path is a resolved export of the importer and is
  /// already projected by [`Self::project_collector_edges`]; this walk only needs to add the
  /// forwarder's *plain-import* targets (which lie on no retained path), so `None` is faithful for
  /// exactly the edges this source owns.
  fn project_excluded_forwarder_edges(
    &self,
    chunk_graph: &ChunkGraph,
    probe_state: &super::order_wrap_state::OrderWrapState,
    module: &NormalModule,
    importer_chunk: ChunkIdx,
    targets: &mut Vec<ModuleIdx>,
  ) {
    let mut visited = FxHashSet::default();
    for rec in &module.import_records {
      if rec.kind != ImportKind::Import
        || !rec.meta.intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly)
      {
        continue;
      }
      let Some(forwarder_idx) = rec.resolved_module else {
        continue;
      };
      if self.link_output.metas[forwarder_idx].is_included {
        continue;
      }
      collect_order_wrap_esm_init_targets(
        &self.link_output.module_table.modules,
        &self.link_output.metas,
        chunk_graph,
        probe_state,
        importer_chunk,
        forwarder_idx,
        None,
        &mut visited,
        targets,
      );
    }
  }

  /// A discovery-only [`OrderWrapState`] that marks exactly the plan's modules order-wrapped, so
  /// `esm_init_target` answers "is this module wrapped by this plan?" during edge projection.
  /// Reuses each module's existing namespace symbol as the wrapper placeholder — the projector only
  /// reads target *identity*, never the wrapper symbol's value — so no facade symbols are minted and
  /// no symbol chunk ownership is touched. Each wrapper is assigned its module's own chunk (exactly
  /// as `place_order_wrap_modules` does after real lowering) so `esm_init_included_in_live_chunk`
  /// answers truthfully — the transitive excluded-hop projection depends on it. Built fresh per
  /// fixpoint round, so it always reflects the current plan with no stale routes.
  fn probe_order_state(
    &self,
    chunk_graph: &ChunkGraph,
    plan: &OrderWrapPlan,
    used_symbol_refs: &UsedSymbolRefsBuilder,
    reverse_static_imports: &IndexVec<ModuleIdx, Vec<ModuleIdx>>,
  ) -> super::order_wrap_state::OrderWrapState {
    let mut probe_state = super::order_wrap_state::OrderWrapState::default();
    for module_idx in plan.modules() {
      if !self.is_order_wrap_eligible(module_idx) {
        // Only `WrapKind::None` ESM/None modules become order wrappers (mirrors `lower_order_state`);
        // an interop wrapper is already visible to `esm_init_target` through its metadata.
        continue;
      }
      let placeholder_wrapper_ref = self.link_output.module_table.modules[module_idx]
        .as_normal()
        .expect("order wrap only applies to normal modules")
        .namespace_object_ref;
      probe_state.insert_order_wrapper_probe(module_idx, placeholder_wrapper_ref);
      if let Some(chunk_idx) = chunk_graph.module_to_chunk[module_idx] {
        probe_state.assign_order_wrapper_chunk(module_idx, chunk_idx);
      }
    }

    // Populate exactly the nested re-export records and per-record overlays `lower_order_state`
    // mints for this plan, so the transitive excluded-hop projection restricts each barrel's walk
    // to its retained re-export path just like the real metadata pass (no over-approximation on
    // retained star re-exports) and the overlay projection sees every eager forwarder's hop. The
    // module's own namespace ref stands in for each not-yet-minted wrapper — projection reads only
    // target identity, never the wrapper symbol's value.
    let input = super::order_wrapping::OrderLoweringInput {
      plan,
      modules: &self.link_output.module_table.modules,
      linking: &self.link_output.metas,
      statements: &self.link_output.stmt_infos,
      export_chains: &self.link_output.normal_symbol_exports_chain_map,
      star_reexport_records_by_imported_symbol: &self
        .link_output
        .star_reexport_records_by_imported_symbol,
      used_symbols: used_symbol_refs,
    };
    let reexport_usage = super::order_wrapping::collect_frozen_reexport_usage(&input);
    probe_state.set_nested_reexport_records(reexport_usage.nested_records().clone());
    super::order_wrapping::populate_order_import_overlays(
      &input,
      &reexport_usage,
      &mut probe_state,
      self.options.code_splitting.is_disabled(),
      reverse_static_imports,
    );
    probe_state
  }

  fn wrap_all_order_analysis(&self, chunk_graph: &ChunkGraph) -> OrderAnalysis {
    let mut plan = OrderWrapPlan::default();
    // Only membership matters here, so one shared visit-state vector lets every module be
    // walked once across all roots instead of once per root.
    let mut states = index_vec![VisitState::Unvisited; self.link_output.module_table.modules.len()];
    for &root in self.link_output.entries.keys() {
      if !self.link_output.metas[root].is_included {
        continue;
      }
      for module_idx in self.expected_order_for_root_with_states(root, &mut states) {
        if self.is_order_wrap_eligible(module_idx) {
          plan.insert(module_idx);
        }
      }
    }
    OrderAnalysis {
      plan,
      import_edges: index_vec![FxHashSet::default(); chunk_graph.chunk_table.len()],
      on_demand: false,
    }
  }

  fn expected_order_for_root(&self, root: ModuleIdx) -> Vec<ModuleIdx> {
    let mut states = index_vec![VisitState::Unvisited; self.link_output.module_table.modules.len()];
    self.expected_order_for_root_with_states(root, &mut states)
  }

  fn expected_order_for_root_with_states(
    &self,
    root: ModuleIdx,
    states: &mut IndexVec<ModuleIdx, VisitState>,
  ) -> Vec<ModuleIdx> {
    let mut order = Vec::new();

    if !self.link_output.module_table[root].is_normal() || states[root] != VisitState::Unvisited {
      return order;
    }

    states[root] = VisitState::Visiting;
    let mut stack = vec![(root, 0usize)];
    while let Some((module_idx, next_import_idx)) = stack.last_mut() {
      let Some(module) = self.link_output.module_table[*module_idx].as_normal() else {
        states[*module_idx] = VisitState::Done;
        stack.pop();
        continue;
      };

      let mut descended = false;
      while *next_import_idx < module.import_records.len() {
        let rec_idx =
          ImportRecordIdx::from_raw(u32::try_from(*next_import_idx).expect("import index fits"));
        let rec = &module.import_records[rec_idx];
        *next_import_idx += 1;

        if rec.kind != ImportKind::Import {
          continue;
        }
        let Some(importee_idx) = rec.resolved_module else { continue };
        if !self.link_output.module_table[importee_idx].is_normal() {
          continue;
        }

        match states[importee_idx] {
          VisitState::Unvisited => {
            states[importee_idx] = VisitState::Visiting;
            stack.push((importee_idx, 0));
            descended = true;
            break;
          }
          VisitState::Visiting | VisitState::Done => {}
        }
      }

      if descended {
        continue;
      }

      let (done_module_idx, _) = stack.pop().expect("stack has a last item");
      if states[done_module_idx] != VisitState::Done {
        states[done_module_idx] = VisitState::Done;
        if self.link_output.metas[done_module_idx].is_included {
          order.push(done_module_idx);
        }
      }
    }

    order
  }

  /// Returns the predicted evaluation order and, for every interop-wrapped module, the eager
  /// module whose top-level walk first reaches it (the host of its first-reach trigger).
  fn actual_order_for_root(
    &self,
    root: ModuleIdx,
    root_chunk: ChunkIdx,
    chunk_graph: &ChunkGraph,
    import_edges: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
  ) -> (Vec<ModuleIdx>, FxHashMap<ModuleIdx, ModuleIdx>) {
    let mut traversal = ActualOrderTraversal {
      chunk_states: index_vec![VisitState::Unvisited; chunk_graph.chunk_table.len()],
      module_states: index_vec![
        VisitState::Unvisited;
        self.link_output.module_table.modules.len()
      ],
      order: Vec::new(),
      trigger_hosts: FxHashMap::default(),
    };

    self.visit_actual_chunk(root_chunk, chunk_graph, import_edges, &mut traversal);
    if !self.link_output.metas[root].wrap_kind().is_none() {
      self.execute_actual_module(root, root, &mut traversal);
    }

    (traversal.order, traversal.trigger_hosts)
  }

  fn visit_actual_chunk(
    &self,
    chunk_idx: ChunkIdx,
    chunk_graph: &ChunkGraph,
    import_edges: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
    traversal: &mut ActualOrderTraversal,
  ) {
    match traversal.chunk_states[chunk_idx] {
      VisitState::Done | VisitState::Visiting => return,
      VisitState::Unvisited => {}
    }
    traversal.chunk_states[chunk_idx] = VisitState::Visiting;

    let mut imports = import_edges[chunk_idx].iter().copied().collect_vec();
    imports
      .sort_unstable_by_key(|importee_chunk| chunk_graph.chunk_table[*importee_chunk].exec_order);
    for importee_chunk in imports {
      self.visit_actual_chunk(importee_chunk, chunk_graph, import_edges, traversal);
    }

    for &module_idx in &chunk_graph.chunk_table[chunk_idx].modules {
      if self.link_output.metas[module_idx].wrap_kind().is_none() {
        self.execute_actual_module(module_idx, module_idx, traversal);
      }
    }

    traversal.chunk_states[chunk_idx] = VisitState::Done;
  }

  fn execute_actual_module(
    &self,
    module_idx: ModuleIdx,
    host: ModuleIdx,
    traversal: &mut ActualOrderTraversal,
  ) {
    match traversal.module_states[module_idx] {
      VisitState::Done | VisitState::Visiting => return,
      VisitState::Unvisited => {}
    }
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return;
    };
    if !self.link_output.metas[module_idx].is_included {
      return;
    }

    traversal.module_states[module_idx] = VisitState::Visiting;
    for rec in &module.import_records {
      if !matches!(rec.kind, ImportKind::Import | ImportKind::Require) {
        continue;
      }
      let Some(importee_idx) = rec.resolved_module else { continue };
      if self.link_output.metas[importee_idx].wrap_kind().is_none() {
        continue;
      }
      self.execute_actual_module(importee_idx, host, traversal);
    }
    traversal.module_states[module_idx] = VisitState::Done;
    if module_idx != host {
      traversal.trigger_hosts.insert(module_idx, host);
    }
    traversal.order.push(module_idx);
  }

  fn at_risk_modules(
    &self,
    expected_order: &[ModuleIdx],
    actual_order: &[ModuleIdx],
    trigger_hosts: &FxHashMap<ModuleIdx, ModuleIdx>,
  ) -> FxHashSet<ModuleIdx> {
    let expected_sensitive_order = self.sensitive_order(expected_order);
    let actual_sensitive_order = self.sensitive_order(actual_order);
    let expected_sensitive_set = expected_sensitive_order.iter().copied().collect::<FxHashSet<_>>();
    let actual_sensitive_set = actual_sensitive_order.iter().copied().collect::<FxHashSet<_>>();
    let actual_positions = actual_sensitive_order
      .iter()
      .copied()
      .enumerate()
      .map(|(position, module_idx)| (module_idx, position))
      .collect::<FxHashMap<_, _>>();
    let mut at_risk = FxHashSet::default();
    // An interop-wrapped at-risk module cannot be delayed itself; delaying the eager module
    // that hosts its first-reach trigger is the only lever, so the signal transfers there.
    let flag = |module_idx: ModuleIdx, at_risk: &mut FxHashSet<ModuleIdx>| {
      if self.is_order_wrap_eligible(module_idx) {
        at_risk.insert(module_idx);
      } else if let Some(&host) = trigger_hosts.get(&module_idx)
        && self.is_order_wrap_eligible(host)
      {
        at_risk.insert(host);
      }
    };

    for module_idx in expected_sensitive_set.symmetric_difference(&actual_sensitive_set) {
      flag(*module_idx, &mut at_risk);
    }

    // Keep both difference directions: phantoms and tree-shaking-omitted sensitive modules.
    // See `strip_plain_chunk_imports`.
    for module_idx in premature_sensitive_modules(&expected_sensitive_order, &actual_positions) {
      flag(module_idx, &mut at_risk);
    }

    at_risk
  }

  fn sensitive_order(&self, order: &[ModuleIdx]) -> Vec<ModuleIdx> {
    let mut seen = FxHashSet::default();
    let mut sensitive_order = Vec::new();
    for module_idx in order.iter().copied().filter(|idx| self.is_order_sensitive(*idx)) {
      if seen.insert(module_idx) {
        sensitive_order.push(module_idx);
      }
    }
    sensitive_order
  }

  fn build_order_wrap_plan(
    &self,
    at_risk: FxHashSet<ModuleIdx>,
    roots: &[RootOrderAnalysis],
    chunk_graph: &ChunkGraph,
    chunk_cycles: &ChunkCycles,
    source_reachable: &FxHashSet<ModuleIdx>,
    reverse_static_imports: &IndexVec<ModuleIdx, Vec<ModuleIdx>>,
  ) -> OrderWrapPlan {
    let mut plan = OrderWrapPlan::default();
    for module_idx in
      at_risk.into_iter().filter(|module_idx| self.is_order_wrap_eligible(*module_idx))
    {
      plan.insert(module_idx);
    }

    // Backward closure of the current plan members over the reverse static-import index: exactly
    // the modules whose static imports reach a member. The plan only grows inside this loop, so the
    // closure grows monotonically — each iteration expands only from the members added since the
    // last one (already-closed seeds are skipped by the insert check) instead of re-walking the
    // graph per candidate module.
    let mut reaches_member = FxHashSet::default();

    loop {
      let mut changed = false;
      changed |= self.close_expected_sensitive_suffixes(roots, &mut plan);
      changed |= self.close_cyclic_chunk_members(chunk_graph, chunk_cycles, &mut plan);

      let current = plan.modules().collect::<FxHashSet<_>>();
      grow_static_import_backward_closure(
        reverse_static_imports,
        plan.modules(),
        &mut reaches_member,
      );

      for module in self.link_output.module_table.modules.iter().filter_map(Module::as_normal) {
        if !source_reachable.contains(&module.idx)
          || !self.is_order_wrap_eligible(module.idx)
          || plan.contains(&module.idx)
        {
          continue;
        }
        if self.statically_imports_wrapped_member(module.idx, &current, &reaches_member)
          || self.top_level_reads_wrapped_export(module.idx, &current)
        {
          changed |= plan.insert(module.idx);
        }
      }

      if !changed {
        break;
      }
    }

    plan
  }

  fn close_cyclic_chunk_members(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_cycles: &ChunkCycles,
    plan: &mut OrderWrapPlan,
  ) -> bool {
    let triggered_sccs = plan
      .modules()
      .filter_map(|module_idx| chunk_graph.module_to_chunk[module_idx])
      .filter_map(|chunk_idx| chunk_cycles.scc_of_chunk.get(&chunk_idx).copied())
      .collect::<FxHashSet<_>>();
    let mut changed = false;

    for scc_idx in triggered_sccs {
      for &chunk_idx in &chunk_cycles.sccs[scc_idx] {
        for &module_idx in &chunk_graph.chunk_table[chunk_idx].modules {
          if self.is_order_sensitive(module_idx) && self.is_order_wrap_eligible(module_idx) {
            changed |= plan.insert(module_idx);
          }
        }
      }
    }

    changed
  }

  fn close_expected_sensitive_suffixes(
    &self,
    roots: &[RootOrderAnalysis],
    plan: &mut OrderWrapPlan,
  ) -> bool {
    let mut changed = false;

    for root in roots {
      let expected_sensitive_order = self.sensitive_order(&root.expected_order);
      let Some(first_wrapped_idx) =
        expected_sensitive_order.iter().position(|module_idx| plan.contains(module_idx))
      else {
        continue;
      };

      // V1 init calls run after the eager chunk body. Once a root wraps an earlier sensitive
      // module, every later eager sensitive module for that root must move behind the same init
      // boundary.
      for module_idx in expected_sensitive_order[first_wrapped_idx..].iter().copied() {
        if self.is_order_wrap_eligible(module_idx) {
          changed |= plan.insert(module_idx);
        }
      }
    }

    changed
  }

  fn source_reachable_modules(&self, roots: &[RootOrderAnalysis]) -> FxHashSet<ModuleIdx> {
    let mut reachable = FxHashSet::default();
    let mut stack = roots.iter().map(|root| root.root).collect_vec();
    while let Some(module_idx) = stack.pop() {
      if !reachable.insert(module_idx) {
        continue;
      }
      let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
        continue;
      };
      for rec in module.import_records.iter().rev() {
        if rec.kind != ImportKind::Import {
          continue;
        }
        let Some(importee_idx) = rec.resolved_module else { continue };
        if self.link_output.module_table[importee_idx].is_normal() {
          stack.push(importee_idx);
        }
      }
    }
    reachable
  }

  fn statically_imports_wrapped_member(
    &self,
    module_idx: ModuleIdx,
    current: &FxHashSet<ModuleIdx>,
    reaches_member: &FxHashSet<ModuleIdx>,
  ) -> bool {
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    let meta = &self.link_output.metas[module_idx];
    if !meta.execution_dependencies.iter().any(|dependency| current.contains(dependency)) {
      return false;
    }
    // `reaches_member` is the precomputed backward closure of `current`, so this membership test
    // answers "does this importee's static-import subtree reach a plan member" without a per-query
    // graph walk.
    module.import_records.iter().any(|rec| {
      rec.kind == ImportKind::Import
        && rec.resolved_module.is_some_and(|importee_idx| reaches_member.contains(&importee_idx))
    })
  }

  fn top_level_reads_wrapped_export(
    &self,
    module_idx: ModuleIdx,
    current: &FxHashSet<ModuleIdx>,
  ) -> bool {
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    let meta = &self.link_output.metas[module_idx];
    if !meta.is_included || !module.meta.contains(EcmaViewMeta::TopLevelImportRead) {
      return false;
    }
    let stmt_infos = &self.link_output.stmt_infos[module_idx];
    stmt_infos.iter_enumerated_without_namespace_stmt().any(|(_, stmt_info)| {
      stmt_info.referenced_symbols.iter().any(|reference_ref| {
        self.reference_touches_wrapped_export(module_idx, reference_ref, current)
      })
    })
  }

  fn reference_touches_wrapped_export(
    &self,
    module_idx: ModuleIdx,
    reference_ref: &SymbolOrMemberExprRef,
    current: &FxHashSet<ModuleIdx>,
  ) -> bool {
    match reference_ref {
      SymbolOrMemberExprRef::Symbol(symbol_ref) => {
        self.symbol_touches_wrapped_export(*symbol_ref, current)
      }
      SymbolOrMemberExprRef::MemberExpr(member_expr_ref) => member_expr_ref
        .resolution(&self.link_output.metas[module_idx].resolved_member_expr_refs)
        .map_or_else(
          || self.symbol_touches_wrapped_export(member_expr_ref.object_ref, current),
          |resolution| {
            resolution
              .resolved
              .is_some_and(|symbol_ref| self.symbol_touches_wrapped_export(symbol_ref, current))
              || resolution
                .depended_refs
                .iter()
                .any(|symbol_ref| self.symbol_touches_wrapped_export(*symbol_ref, current))
          },
        ),
    }
  }

  fn symbol_touches_wrapped_export(
    &self,
    symbol_ref: SymbolRef,
    current: &FxHashSet<ModuleIdx>,
  ) -> bool {
    let canonical_ref = self.link_output.symbol_db.canonical_ref_for(symbol_ref);
    current.contains(&symbol_ref.owner)
      || current.contains(&canonical_ref.owner)
      || self
        .link_output
        .normal_symbol_exports_chain_map
        .get(&symbol_ref)
        .is_some_and(|refs| refs.iter().any(|ref_| current.contains(&ref_.owner)))
      || self
        .link_output
        .normal_symbol_exports_chain_map
        .get(&canonical_ref)
        .is_some_and(|refs| refs.iter().any(|ref_| current.contains(&ref_.owner)))
  }

  fn is_order_sensitive(&self, module_idx: ModuleIdx) -> bool {
    if module_idx == self.link_output.runtime.id() {
      return false;
    }
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    let meta = &self.link_output.metas[module.idx];
    if !meta.is_included {
      return false;
    }

    let has_intrinsic_effect = module.meta.contains(EcmaViewMeta::ExecutionOrderSensitive);

    has_intrinsic_effect || self.eagerly_triggers_interop_module(module_idx)
  }

  /// A retained static import of an interop module runs its wrapper inside the importer.
  /// Mark the importer sensitive so code splitting can delay that trigger.
  fn eagerly_triggers_interop_module(&self, module_idx: ModuleIdx) -> bool {
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    let meta = &self.link_output.metas[module_idx];
    if !meta.is_included {
      return false;
    }
    self.link_output.stmt_infos[module_idx].iter_enumerated_without_namespace_stmt().any(
      |(stmt_info_idx, stmt_info)| {
        meta.stmt_info_included.has_bit(stmt_info_idx)
          && stmt_info.import_records.iter().any(|rec_idx| {
            let rec = &module.import_records[*rec_idx];
            // Only static `import` records run eagerly at the importer's position; a `require`
            // inside a function body is call-time and must not be treated as a top-level trigger.
            rec.kind == ImportKind::Import
              && rec.resolved_module.is_some_and(|importee_idx| {
                self.link_output.module_table[importee_idx].as_normal().is_some_and(|_| {
                  !matches!(self.link_output.metas[importee_idx].wrap_kind(), WrapKind::None)
                })
              })
          })
      },
    )
  }

  fn is_order_wrap_eligible(&self, module_idx: ModuleIdx) -> bool {
    if module_idx == self.link_output.runtime.id() {
      return false;
    }
    if !self.link_output.metas[module_idx].is_included {
      return false;
    }
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    matches!(module.exports_kind, ExportsKind::Esm | ExportsKind::None)
      && matches!(self.link_output.metas[module_idx].wrap_kind(), WrapKind::None)
  }
}

/// Importee → normal importers holding a static `import` record to it: the exact reverse of the
/// edges the plan-closure reachability walks traverse (only normal modules expand; any resolved
/// importee is a node). Plan-independent, so one index serves every fixpoint round.
pub(super) fn reverse_static_import_index(
  modules: &rolldown_common::IndexModules,
) -> IndexVec<ModuleIdx, Vec<ModuleIdx>> {
  let mut reverse: IndexVec<ModuleIdx, Vec<ModuleIdx>> = index_vec![Vec::new(); modules.len()];
  for module in modules.iter().filter_map(Module::as_normal) {
    for rec in &module.import_records {
      if rec.kind == ImportKind::Import
        && let Some(importee_idx) = rec.resolved_module
      {
        reverse[importee_idx].push(module.idx);
      }
    }
  }
  reverse
}

/// Grow `reached` to the backward closure of `seeds` over the reverse static-import index:
/// afterwards `reached` holds exactly the modules from which some seed is reachable through static
/// `import` edges (seeds included — a module reaches itself). Monotone: seeds already in `reached`
/// are skipped, so repeated calls with a growing seed set expand only the new frontier.
pub(super) fn grow_static_import_backward_closure(
  reverse_static_imports: &IndexVec<ModuleIdx, Vec<ModuleIdx>>,
  seeds: impl Iterator<Item = ModuleIdx>,
  reached: &mut FxHashSet<ModuleIdx>,
) {
  let mut stack = seeds.filter(|seed| reached.insert(*seed)).collect_vec();
  while let Some(module_idx) = stack.pop() {
    for &importer_idx in &reverse_static_imports[module_idx] {
      if reached.insert(importer_idx) {
        stack.push(importer_idx);
      }
    }
  }
}

/// Nontrivial strongly connected components (two or more chunks, or a self-loop) of the
/// predicted chunk-import graph. Both cycle consumers read it: the per-root bailout and the
/// plan closure.
struct ChunkCycles {
  scc_of_chunk: FxHashMap<ChunkIdx, usize>,
  sccs: Vec<Vec<ChunkIdx>>,
}

impl ChunkCycles {
  fn from_import_edges(import_edges: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>) -> Self {
    let mut graph = DiGraphMap::<ChunkIdx, ()>::new();
    for (importer_idx, importees) in import_edges.iter_enumerated() {
      graph.add_node(importer_idx);
      for &importee_idx in importees {
        graph.add_edge(importer_idx, importee_idx, ());
      }
    }
    let mut scc_of_chunk = FxHashMap::default();
    let mut sccs = Vec::new();
    for scc in petgraph::algo::tarjan_scc(&graph) {
      if scc.len() < 2 && !import_edges[scc[0]].contains(&scc[0]) {
        continue;
      }
      for &chunk_idx in &scc {
        scc_of_chunk.insert(chunk_idx, sccs.len());
      }
      sccs.push(scc);
    }
    Self { scc_of_chunk, sccs }
  }

  /// Whether any static chunk cycle is reachable from `root_chunk` over the predicted edges.
  fn reachable_from(
    &self,
    root_chunk: ChunkIdx,
    import_edges: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
  ) -> bool {
    if self.scc_of_chunk.is_empty() {
      return false;
    }
    let mut visited = FxHashSet::default();
    let mut pending = vec![root_chunk];
    while let Some(chunk_idx) = pending.pop() {
      if !visited.insert(chunk_idx) {
        continue;
      }
      if self.scc_of_chunk.contains_key(&chunk_idx) {
        return true;
      }
      pending.extend(import_edges[chunk_idx].iter().copied());
    }
    false
  }
}

fn premature_sensitive_modules(
  expected_sensitive_order: &[ModuleIdx],
  actual_positions: &FxHashMap<ModuleIdx, usize>,
) -> FxHashSet<ModuleIdx> {
  let mut premature_modules = FxHashSet::default();
  let mut latest_predecessor_actual_position = None::<usize>;

  for module_idx in expected_sensitive_order {
    let Some(&actual_position) = actual_positions.get(module_idx) else {
      continue;
    };

    if latest_predecessor_actual_position.is_some_and(|latest| actual_position < latest) {
      premature_modules.insert(*module_idx);
    }
    latest_predecessor_actual_position = Some(
      latest_predecessor_actual_position
        .map_or(actual_position, |latest| latest.max(actual_position)),
    );
  }

  premature_modules
}
