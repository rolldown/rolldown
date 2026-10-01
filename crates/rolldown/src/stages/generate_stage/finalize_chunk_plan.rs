#[cfg(debug_assertions)]
use rolldown_common::{ChunkIdx, ChunkKind};
use rolldown_common::{
  ImportKind, ImportRecordMeta, ModuleIdx, SymbolRef, UsedSymbolRefsBuilder, WrapKind,
};
use rolldown_error::BuildResult;
use rustc_hash::FxHashSet;

use crate::{
  chunk_graph::ChunkGraph,
  utils::chunk::validate_options_for_multi_chunk_output::validate_options_for_multi_chunk_output,
};

use super::GenerateStage;
#[cfg(debug_assertions)]
use super::order_analysis::OrderWrapPlan;
use super::order_wrap_state::OrderWrapState;

impl GenerateStage<'_> {
  /// Under wrap-all strict execution order, a side-effect-free indirect re-exporter retained only
  /// for its own effects (`import './configure.js'; import { a } from './a.js'; export { a }`)
  /// would initialize, and load, every module it forwards. Mark its records that only forward
  /// bindings of side-effect-free modules so consumers initialize those owners directly.
  fn mark_effect_only_forwarders(&self, order_state: &mut OrderWrapState) {
    if !self.options.is_strict_execution_order_enabled()
      || self.options.experimental.is_on_demand_wrapping_enabled()
    {
      return;
    }
    let modules = &self.link_output.module_table.modules;
    let metas = &self.link_output.metas;
    let is_plain_pure_module = |idx: ModuleIdx| {
      matches!(metas[idx].wrap_kind(), WrapKind::None)
        && modules[idx].as_normal().is_some_and(|m| !m.side_effects.has_side_effects())
    };
    for &module_idx in &self.link_output.indirect_reexport_body_modules {
      let (Some(module), meta) = (modules[module_idx].as_normal(), &metas[module_idx]) else {
        continue;
      };
      if !meta.is_included
        || !matches!(meta.wrap_kind(), WrapKind::None)
        || meta.has_dynamic_exports
        || meta.namespace_included
        || !module.named_exports.values().all(|e| module.named_imports.contains_key(&e.referenced))
      {
        continue;
      }
      // Imported bindings read by the forwarder's own retained statements keep their records.
      let locally_read: FxHashSet<SymbolRef> = self.link_output.stmt_infos[module_idx]
        .iter_enumerated()
        .filter(|(idx, info)| {
          meta.stmt_info_included.has_bit(*idx) && info.import_records.is_empty()
        })
        .flat_map(|(_, info)| info.referenced_symbols.iter().map(|r| *r.symbol_ref()))
        .collect();
      let forwarding_only_records =
        module.import_records.iter_enumerated().filter_map(|(idx, rec)| {
          let mut locals =
            module.named_imports.iter().filter(|(_, import)| import.record_idx == idx).peekable();
          let forwards_only = rec.kind == ImportKind::Import
            && !rec
              .meta
              .intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly)
            && rec.resolved_module.is_some_and(is_plain_pure_module)
            && locals.peek().is_some()
            && locals.all(|(local_ref, _)| !locally_read.contains(local_ref));
          forwards_only.then_some(idx)
        });
      order_state.set_effect_only_forwarder(module_idx, forwarding_only_records);
    }
  }

  /// Apply order wrapping and entry facades before deriving final output metadata.
  pub(super) fn finalize_chunk_plan(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    used_symbol_refs_builder: &mut UsedSymbolRefsBuilder,
  ) -> BuildResult<OrderWrapState> {
    // The order analysis reuses cross-chunk linking logic, which reads finalized namespace and
    // external-export facts. Prepare those inputs on the provisional topology first.
    self.find_entry_level_external_module(chunk_graph);
    let mut order_state = OrderWrapState::default();
    self.mark_effect_only_forwarders(&mut order_state);
    self.finalized_module_namespace_ref_usage(chunk_graph, &order_state);

    let mut order_analysis = self.analyze_execution_order(chunk_graph, used_symbol_refs_builder);
    let runtime_evicted_for_analysis = self.options.experimental.is_on_demand_wrapping_enabled()
      && !self.options.code_splitting.is_disabled()
      && chunk_graph.module_to_chunk[self.link_output.runtime.id()]
        .is_some_and(|chunk_idx| chunk_graph.chunk_table[chunk_idx].modules.len() > 1)
      && order_analysis.as_ref().is_some_and(|analysis| {
        !analysis.plan.is_empty()
          || self
            .pre_chunk_order_state(used_symbol_refs_builder)
            .has_consumer_local_reexport_routes()
      });
    if runtime_evicted_for_analysis {
      self.ensure_runtime_module_for_order_wraps(chunk_graph);
      chunk_graph.rebuild_sorted_chunk_idx_vec(true);
      self.find_entry_level_external_module(chunk_graph);
      self.finalized_module_namespace_ref_usage(chunk_graph, &order_state);
      order_analysis = self.analyze_execution_order(chunk_graph, used_symbol_refs_builder);
    }
    if let Some(analysis) = &order_analysis
      && self.apply_order_wraps(chunk_graph, analysis, used_symbol_refs_builder, &mut order_state)
    {
      #[cfg(debug_assertions)]
      self.assert_order_wrap_plan_applied(chunk_graph, &analysis.plan, &order_state);
      self.find_entry_level_external_module(chunk_graph);
      self.finalized_module_namespace_ref_usage(chunk_graph, &order_state);
    }

    if runtime_evicted_for_analysis
      && order_analysis.as_ref().is_some_and(|analysis| analysis.plan.is_empty())
      && !order_state.has_consumer_local_reexport_routes()
    {
      self.fold_runtime_chunk_after_order_lowering(chunk_graph, &order_state);
      chunk_graph.sort_chunk_modules(self.link_output, self.options);
      self.renumber_live_chunks(chunk_graph);
    }

    // The runtime sweep must observe the final namespace/external facts above. Order wrappers
    // carry their runtime demand in `OrderWrapState`, outside the link-stage metadata inspected
    // by the sweep, so conservatively keep the runtime whenever that synthetic demand exists.
    if order_state.required_runtime_helpers().is_empty() {
      let runtime_idx = self.link_output.runtime.id();
      let runtime_chunk_before = chunk_graph.module_to_chunk[runtime_idx];
      self.sweep_unused_runtime_module(chunk_graph, used_symbol_refs_builder);
      if runtime_chunk_before.is_some() && chunk_graph.module_to_chunk[runtime_idx].is_none() {
        // The sweep removed the runtime from its chunk. When that chunk is still live (the runtime
        // co-hosted with user modules), its `modules[0]` changed, so the `exec_order` the
        // provisional `assign_chunk_exec_orders` in `generate_chunks` derived from the runtime
        // (`exec_order` 0) is now stale and would sort the chunk ahead of chunks it should follow.
        // Re-derive every live chunk's `exec_order` from its current lead module before rebuilding
        // the sorted list, which restores the ordering the pre-#10104 pipeline produced by sweeping
        // before assignment. See `assign_chunk_exec_orders` for how this composes with the
        // strict-only `renumber_live_chunks`.
        self.assign_chunk_exec_orders(chunk_graph);
        chunk_graph.rebuild_sorted_chunk_idx_vec(true);
      }
    }

    let rendered_chunk_count = chunk_graph
      .chunk_table
      .len()
      .saturating_sub(chunk_graph.post_chunk_optimization_operations.len());
    if rendered_chunk_count > 1 {
      validate_options_for_multi_chunk_output(self.options)?;
    }

    Ok(order_state)
  }

  #[cfg(debug_assertions)]
  fn assert_order_wrap_plan_applied(
    &self,
    chunk_graph: &ChunkGraph,
    plan: &OrderWrapPlan,
    order_state: &OrderWrapState,
  ) {
    if plan.is_empty() {
      return;
    }

    for module_idx in plan.modules() {
      let meta = &self.link_output.metas[module_idx];
      debug_assert!(matches!(meta.wrap_kind(), WrapKind::None));
      debug_assert!(meta.wrapper_ref.is_none());
      debug_assert!(meta.wrapper_stmt_info.is_none());
      debug_assert!(order_state.has_order_wrapper(module_idx));
      debug_assert!(matches!(
        order_state.esm_init_target(module_idx, meta).map(|target| target.origin),
        Some(super::order_wrap_state::EsmInitOrigin::ExecutionOrder)
      ));
      debug_assert!(chunk_graph.module_to_chunk[module_idx].is_some_and(|chunk_idx| {
        is_rendered_chunk(chunk_graph, chunk_idx)
          && chunk_graph.chunk_table[chunk_idx].modules.contains(&module_idx)
      }));

      if self.link_output.entries.contains_key(&module_idx) {
        let entry_chunk_idx = chunk_graph.entry_module_to_entry_chunk[&module_idx];
        debug_assert!(is_rendered_chunk(chunk_graph, entry_chunk_idx));
        match chunk_graph.chunk_table[entry_chunk_idx].kind {
          // A dynamic entry merged into a user-defined entry chunk keeps that host chunk as
          // its entry chunk (the facade stays eliminated); the host must contain the module.
          ChunkKind::EntryPoint { module, .. } => debug_assert!(
            module == module_idx
              || chunk_graph.chunk_table[entry_chunk_idx].modules.contains(&module_idx)
          ),
          ChunkKind::Common => {
            debug_assert!(chunk_graph.chunk_table[entry_chunk_idx].modules.contains(&module_idx));
          }
        }
      }
    }
    let runtime_idx = self.link_output.runtime.id();
    if self.link_output.metas[runtime_idx].is_included
      || !order_state.required_runtime_helpers().is_empty()
    {
      debug_assert!(chunk_graph.module_to_chunk[runtime_idx].is_some_and(|chunk_idx| {
        is_rendered_chunk(chunk_graph, chunk_idx)
          && chunk_graph.chunk_table[chunk_idx].modules.contains(&runtime_idx)
      }));
    }

    let mut sorted_chunks = FxHashSet::default();
    for chunk_idx in &chunk_graph.sorted_chunk_idx_vec {
      let inserted = sorted_chunks.insert(*chunk_idx);
      debug_assert!(inserted, "chunk appears twice in sorted order");
    }
    for (chunk_idx, _) in chunk_graph.chunk_table.iter_enumerated() {
      if is_rendered_chunk(chunk_graph, chunk_idx) {
        debug_assert!(
          sorted_chunks.contains(&chunk_idx),
          "rendered chunk missing from sorted order"
        );
      }
    }
  }
}

#[cfg(debug_assertions)]
fn is_rendered_chunk(chunk_graph: &ChunkGraph, chunk_idx: ChunkIdx) -> bool {
  !chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
}
