use rolldown_common::UsedSymbolRefsBuilder;
#[cfg(debug_assertions)]
use rolldown_common::{ChunkIdx, ChunkKind, WrapKind};
use rolldown_devtools::trace_action_enabled;
use rolldown_error::BuildResult;
#[cfg(debug_assertions)]
use rustc_hash::FxHashSet;

use crate::{
  chunk_graph::ChunkGraph,
  utils::chunk::validate_options_for_multi_chunk_output::validate_options_for_multi_chunk_output,
};

use super::GenerateStage;
use super::order_analysis::OrderAnalysis;
#[cfg(debug_assertions)]
use super::order_analysis::OrderWrapPlan;
use super::strict_execution_order_trace::strict_execution_order_trace_requested;

impl GenerateStage<'_> {
  /// Finalize topology-changing generate-stage decisions before deriving output metadata.
  ///
  /// `generate_chunks` produces the provisional layout used by the order analysis. Applying the
  /// resulting plan may add or restore facades, move the runtime, and renumber chunks, so entry
  /// metadata, namespace usage, and output-shape validation must observe this final topology.
  ///
  /// See `internal-docs/code-splitting/implementation.md`.
  pub(super) fn finalize_chunk_plan(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    used_symbol_refs: &mut UsedSymbolRefsBuilder,
  ) -> BuildResult<Option<OrderAnalysis>> {
    // The order analysis reuses cross-chunk linking logic, which reads finalized namespace and
    // external-export facts. Prepare those inputs on the provisional topology first.
    self.find_entry_level_external_module(chunk_graph);
    self.finalized_module_namespace_ref_usage();

    let order_analysis = self.analyze_execution_order(chunk_graph, used_symbol_refs);
    if let Some(analysis) = &order_analysis
      && !analysis.plan.is_empty()
    {
      self.apply_order_wraps(chunk_graph, &analysis.plan, used_symbol_refs);
      #[cfg(debug_assertions)]
      self.assert_order_wrap_plan_applied(chunk_graph, &analysis.plan);

      // Applying the plan can replace or restore entry facades and extend namespace inclusion.
      // Recompute topology-derived facts on the graph that will actually be rendered.
      self.find_entry_level_external_module(chunk_graph);
      self.finalized_module_namespace_ref_usage();
    }

    let rendered_chunk_count = chunk_graph
      .chunk_table
      .len()
      .saturating_sub(chunk_graph.post_chunk_optimization_operations.len());
    if rendered_chunk_count > 1 {
      validate_options_for_multi_chunk_output(self.options)?;
    }

    if strict_execution_order_trace_requested(self.options.devtools) && trace_action_enabled!() {
      Ok(order_analysis)
    } else {
      Ok(None)
    }
  }

  #[cfg(debug_assertions)]
  fn assert_order_wrap_plan_applied(&self, chunk_graph: &ChunkGraph, plan: &OrderWrapPlan) {
    if plan.is_empty() {
      return;
    }

    for module_idx in plan.modules() {
      let meta = &self.link_output.metas[module_idx];
      debug_assert!(matches!(meta.wrap_kind(), WrapKind::Esm));
      debug_assert!(meta.hoist_esm_wrapper);
      debug_assert!(
        meta.wrapper_stmt_info.is_some_and(|stmt_idx| meta.stmt_info_included.has_bit(stmt_idx))
      );
      debug_assert!(chunk_graph.module_to_chunk[module_idx].is_some_and(|chunk_idx| {
        is_rendered_chunk(chunk_graph, chunk_idx)
          && chunk_graph.chunk_table[chunk_idx].modules.contains(&module_idx)
      }));

      if self.link_output.entries.contains_key(&module_idx) {
        let entry_chunk_idx = chunk_graph.entry_module_to_entry_chunk[&module_idx];
        debug_assert!(is_rendered_chunk(chunk_graph, entry_chunk_idx));
        match chunk_graph.chunk_table[entry_chunk_idx].kind {
          ChunkKind::EntryPoint { module, .. } => debug_assert_eq!(module, module_idx),
          ChunkKind::Common => {
            debug_assert!(chunk_graph.chunk_table[entry_chunk_idx].modules.contains(&module_idx));
          }
        }
      }
    }

    let runtime_idx = self.link_output.runtime.id();
    if self.link_output.metas[runtime_idx].is_included {
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
