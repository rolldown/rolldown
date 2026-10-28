#[cfg(debug_assertions)]
use rolldown_common::{ChunkIdx, ChunkKind, WrapKind};
use rolldown_common::{UsedSymbolRefsBuilder, UsedSymbolRefsView};
use rolldown_error::BuildResult;
#[cfg(debug_assertions)]
use rustc_hash::FxHashSet;

use crate::{
  chunk_graph::ChunkGraph,
  utils::chunk::validate_options_for_multi_chunk_output::validate_options_for_multi_chunk_output,
};

use super::compute_cross_chunk_links::{CrossChunkLinkState, FinalEsmInitMetadataAvailability};
#[cfg(debug_assertions)]
use super::order_analysis::OrderWrapPlan;
use super::order_wrap_state::OrderWrapState;
use super::{FinalEsmInitMetadata, GenerateStage, Sealed};

pub(super) struct FinalizedChunkPlan {
  pub(super) order_state: OrderWrapState,
  pub(super) final_esm_init_metadata: Sealed<FinalEsmInitMetadata>,
  pub(super) link_state: CrossChunkLinkState,
  pub(super) import_order: Option<super::compute_cross_chunk_links::ChunkImportOrder>,
}

impl GenerateStage<'_> {
  /// Finalize entry facades, runtime liveness and the links for the completed chunk layout.
  pub(super) fn finalize_chunk_plan(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    used_symbol_refs_builder: &mut UsedSymbolRefsBuilder,
  ) -> BuildResult<FinalizedChunkPlan> {
    // The order analysis reuses cross-chunk linking logic, which reads finalized namespace and
    // external-export facts. Prepare those inputs on the provisional topology first.
    self.find_entry_level_external_module(chunk_graph);
    let mut order_state = OrderWrapState::default();
    self.finalized_module_namespace_ref_usage(chunk_graph, &order_state);

    let order_analysis = self.analyze_execution_order(chunk_graph, used_symbol_refs_builder);
    if let Some(analysis) = &order_analysis
      && self.apply_order_wraps(chunk_graph, analysis, used_symbol_refs_builder, &mut order_state)
    {
      #[cfg(debug_assertions)]
      self.assert_order_wrap_plan_applied(chunk_graph, &analysis.plan, &order_state);
      self.find_entry_level_external_module(chunk_graph);
      self.finalized_module_namespace_ref_usage(chunk_graph, &order_state);
    }

    // The runtime sweep must observe the final namespace/external facts above. Order wrappers
    // carry their runtime demand in `OrderWrapState`, outside the link-stage metadata inspected
    // by the sweep, so conservatively keep the runtime whenever that synthetic demand exists.
    if order_state.required_runtime_helpers().is_empty() {
      let runtime_idx = self.link_output.runtime.id();
      let runtime_chunk_before = chunk_graph.module_to_chunk[runtime_idx];
      self.sweep_unused_runtime_module(chunk_graph, used_symbol_refs_builder);
      if runtime_chunk_before.is_some() && chunk_graph.module_to_chunk[runtime_idx].is_none() {
        // Removing a co-hosted runtime changes the chunk's lead module and its execution order.
        self.assign_chunk_exec_orders(chunk_graph);
        chunk_graph.rebuild_sorted_chunk_idx_vec(true);
      }
    }

    let (mut final_esm_init_metadata, mut link_state) =
      self.compute_final_chunk_links(chunk_graph, used_symbol_refs_builder.view(), &order_state);
    // See internal-docs/code-splitting/implementation.md: derived exports include generated
    // bindings that strict entries must keep behind a public facade.
    if self.preserve_strict_entry_signatures(
      chunk_graph,
      used_symbol_refs_builder.view(),
      &link_state,
    ) {
      chunk_graph.sort_chunk_modules(self.link_output, self.options);
      self.assign_chunk_exec_orders(chunk_graph);
      chunk_graph.rebuild_sorted_chunk_idx_vec(true);
      self.find_entry_level_external_module(chunk_graph);
      self.finalized_module_namespace_ref_usage(chunk_graph, &order_state);
      // Release the old tables before allocating their replacements.
      drop((final_esm_init_metadata, link_state));
      (final_esm_init_metadata, link_state) =
        self.compute_final_chunk_links(chunk_graph, used_symbol_refs_builder.view(), &order_state);
    }

    // Selection can add registry helpers, but it does not change user-code liveness.
    // Keep runtime separate when records will be copied into multiple files.
    let has_inline_records =
      self.select_inline_common_chunks(chunk_graph, &link_state, &mut order_state);
    if has_inline_records {
      link_state = self.compute_cross_chunk_link_state(
        chunk_graph,
        used_symbol_refs_builder.view(),
        &order_state,
        FinalEsmInitMetadataAvailability::Sealed(&final_esm_init_metadata),
      );
    }

    // The consumer set is now complete, including CJS interop and order wrappers. Select a
    // host against these links, then retain the approved order when ownership changes.
    #[cfg(debug_assertions)]
    let imports_before_fold = link_state.static_importee_table(chunk_graph);
    // Inlining changes physical imports later. Otherwise compute their order once, even
    // when the runtime cannot merge, and reuse it when committing the final links.
    let mut import_order =
      (!has_inline_records).then(|| self.compute_chunk_import_order(chunk_graph, &link_state));
    let placement = import_order.as_mut().and_then(|import_order| {
      self.fold_final_runtime_chunk(chunk_graph, &link_state, &order_state, import_order)
    });
    if let Some(placement) = placement {
      #[cfg(not(debug_assertions))]
      let _ = placement;
      drop((final_esm_init_metadata, link_state));
      (final_esm_init_metadata, link_state) =
        self.compute_final_chunk_links(chunk_graph, used_symbol_refs_builder.view(), &order_state);
      #[cfg(debug_assertions)]
      {
        let imports_after_fold = link_state.static_importee_table(chunk_graph);
        for (idx, imports) in imports_after_fold.iter_enumerated() {
          if !chunk_graph.chunk_is_live(idx) {
            continue;
          }
          let mut expected = imports_before_fold[idx].clone();
          expected.remove(&placement.runtime_chunk);
          if idx != placement.host && placement.consumers.contains(&idx) {
            expected.insert(placement.host);
          }
          debug_assert_eq!(
            imports, &expected,
            "runtime folding changed an unapproved user-chunk edge"
          );
        }
        link_state.assert_runtime_importers(
          self.link_output.runtime.id(),
          &self.link_output.symbol_db,
          &placement.consumers,
        );
      }
    }

    let rendered_chunk_count = chunk_graph
      .chunk_table
      .len()
      .saturating_sub(chunk_graph.post_chunk_optimization_operations.len());
    if rendered_chunk_count > 1 {
      validate_options_for_multi_chunk_output(self.options)?;
    }

    Ok(FinalizedChunkPlan { order_state, final_esm_init_metadata, link_state, import_order })
  }

  fn compute_final_chunk_links(
    &self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: UsedSymbolRefsView<'_>,
    order_state: &OrderWrapState,
  ) -> (Sealed<FinalEsmInitMetadata>, CrossChunkLinkState) {
    let metadata = self.compute_wrapped_esm_init_metadata(
      &self.ast_table,
      chunk_graph,
      order_state,
      used_symbol_refs,
    );
    let links = self.compute_cross_chunk_link_state(
      chunk_graph,
      used_symbol_refs,
      order_state,
      FinalEsmInitMetadataAvailability::Sealed(&metadata),
    );
    (metadata, links)
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
