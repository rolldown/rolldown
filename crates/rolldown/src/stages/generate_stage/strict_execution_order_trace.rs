use itertools::Itertools;
use rolldown_common::{ChunkIdx, ImportKind, Module, ModuleIdx, SymbolRef, WrapKind};
use rolldown_devtools::{action, trace_action, trace_action_enabled};
use rustc_hash::FxHashSet;

use crate::{
  chunk_graph::ChunkGraph,
  module_finalizers::{
    WrappedEsmInitTargetContext, collect_wrapped_esm_init_targets_for_import_record,
  },
};

use super::{
  GenerateStage,
  order_analysis::{OrderAnalysis, OrderWrapReason},
};

const STRICT_EXECUTION_ORDER_TRACE_ENV: &str = "ROLLDOWN_STRICT_ORDER_TRACE";

impl GenerateStage<'_> {
  /// Emit the versioned diagnostic snapshot after init metadata and final chunk links exist.
  ///
  /// See `internal-docs/devtools/implementation.md`.
  pub(super) fn trace_action_strict_execution_order_plan_ready(
    &self,
    chunk_graph: &ChunkGraph,
    order_state: &super::order_wrap_state::OrderWrapState,
    analysis: Option<OrderAnalysis>,
  ) {
    let Some(analysis) = analysis else { return };
    if !strict_execution_order_trace_requested(self.options.devtools) || !trace_action_enabled!() {
      return;
    }

    let rendered_modules = chunk_graph
      .sorted_chunk_idx_vec
      .iter()
      .copied()
      .filter(|&chunk_idx| is_rendered_chunk(chunk_graph, chunk_idx))
      .flat_map(|chunk_idx| chunk_graph.chunk_table[chunk_idx].modules.iter().copied())
      .collect::<FxHashSet<_>>();

    let roots = analysis
      .roots
      .iter()
      .map(|root| action::StrictExecutionOrderRoot {
        root_module_id: self.module_id(root.root),
        expected_order: self.module_ids(&root.expected_order),
        predicted_pre_wrap_order: self.module_ids(&root.actual_order),
        at_risk_modules: root
          .at_risk
          .iter()
          .copied()
          .sorted_unstable_by_key(|module_idx| {
            self.link_output.module_table[*module_idx].exec_order()
          })
          .map(|module_idx| self.module_id(module_idx))
          .collect(),
      })
      .collect();

    let plan_modules = analysis
      .plan
      .modules()
      .sorted_unstable_by_key(|module_idx| self.link_output.module_table[*module_idx].exec_order())
      .map(|module_idx| action::StrictExecutionOrderPlanModule {
        module_id: self.module_id(module_idx),
        reasons: analysis
          .plan
          .reasons(module_idx)
          .iter()
          .copied()
          .map(OrderWrapReason::as_static_str)
          .map(str::to_string)
          .collect(),
      })
      .collect();

    let included_modules = self
      .link_output
      .module_table
      .modules
      .iter_enumerated()
      .filter_map(|(module_idx, module)| {
        let Module::Normal(_) = module else { return None };
        let meta = &self.link_output.metas[module_idx];
        meta.is_included.then(|| action::StrictExecutionOrderModule {
          module_id: module.id().to_string(),
          original_wrap_kind: wrap_kind_name(meta.original_wrap_kind()),
          final_wrap_kind: wrap_kind_name(meta.wrap_kind()),
          final_chunk_id: chunk_graph.module_to_chunk[module_idx]
            .filter(|&chunk_idx| is_rendered_chunk(chunk_graph, chunk_idx))
            .map(ChunkIdx::raw),
          entry_chunk_id: chunk_graph
            .entry_module_to_entry_chunk
            .get(&module_idx)
            .copied()
            .filter(|&chunk_idx| is_rendered_chunk(chunk_graph, chunk_idx))
            .map(ChunkIdx::raw),
          wrapper_included: self.wrapper_is_included(module_idx),
          tla_tainted: meta.is_tla_or_contains_tla_dependency,
        })
      })
      .collect();

    let rendered_chunks = chunk_graph
      .sorted_chunk_idx_vec
      .iter()
      .filter(|&&chunk_idx| is_rendered_chunk(chunk_graph, chunk_idx))
      .map(|&chunk_idx| {
        let chunk = &chunk_graph.chunk_table[chunk_idx];
        action::StrictExecutionOrderChunk {
          chunk_id: chunk_idx.raw(),
          module_ids: self.module_ids(&chunk.modules),
          static_chunk_imports: chunk
            .cross_chunk_imports
            .iter()
            .copied()
            .filter(|&chunk_idx| is_rendered_chunk(chunk_graph, chunk_idx))
            .map(ChunkIdx::raw)
            .collect(),
          dynamic_chunk_imports: chunk
            .cross_chunk_dynamic_imports
            .iter()
            .copied()
            .filter(|&chunk_idx| is_rendered_chunk(chunk_graph, chunk_idx))
            .map(ChunkIdx::raw)
            .collect(),
        }
      })
      .collect();

    let init_obligations =
      self.strict_execution_order_init_obligations(chunk_graph, &rendered_modules, order_state);

    trace_action!(action::StrictExecutionOrderPlanReady {
      action: "StrictExecutionOrderPlanReady",
      version: 1,
      roots,
      plan_modules,
      included_modules,
      rendered_chunks,
      init_obligations,
    });
  }

  fn strict_execution_order_init_obligations(
    &self,
    chunk_graph: &ChunkGraph,
    rendered_modules: &FxHashSet<ModuleIdx>,
    order_state: &super::order_wrap_state::OrderWrapState,
  ) -> Vec<action::StrictExecutionOrderInitObligation> {
    let mut obligations = Vec::new();

    for (importer_idx, importer) in self
      .link_output
      .module_table
      .modules
      .iter_enumerated()
      .filter_map(|(idx, module)| module.as_normal().map(|module| (idx, module)))
    {
      let importer_meta = &self.link_output.metas[importer_idx];
      if !importer_meta.is_included || !rendered_modules.contains(&importer_idx) {
        continue;
      }
      let mut recorded_targets = FxHashSet::default();

      for (stmt_idx, stmt_info) in
        self.link_output.stmt_infos[importer_idx].iter_enumerated_without_namespace_stmt()
      {
        if importer_meta.stmt_info_included.has_bit(stmt_idx) {
          for &rec_idx in &stmt_info.import_records {
            let record = &importer.import_records[rec_idx];
            if record.kind != ImportKind::Import {
              continue;
            }
            for importee_idx in collect_wrapped_esm_init_targets_for_import_record(
              &WrappedEsmInitTargetContext {
                importer,
                importer_meta,
                modules: &self.link_output.module_table.modules,
                metas: &self.link_output.metas,
                symbol_db: &self.link_output.symbol_db,
                order_wrap_state: order_state,
              },
              rec_idx,
              |wrapper_ref| {
                self.wrapper_is_reachable_from_module_chunk(chunk_graph, importer_idx, wrapper_ref)
              },
              |forwarding_module_idx| {
                chunk_graph.module_to_chunk[forwarding_module_idx]
                  == chunk_graph.module_to_chunk[importer_idx]
              },
            ) {
              let importee_meta = &self.link_output.metas[importee_idx];
              if !rendered_modules.contains(&importee_idx)
                || !recorded_targets.insert(("direct-import", importee_idx))
              {
                continue;
              }
              obligations.push(self.strict_execution_order_init_obligation(
                "direct-import",
                importer_idx,
                importee_idx,
                importee_meta.is_tla_or_contains_tla_dependency,
              ));
            }
          }
        } else if let Some(targets) = importer_meta.transitive_esm_init_targets.get(&stmt_idx) {
          for &importee_idx in targets {
            if !rendered_modules.contains(&importee_idx)
              || !recorded_targets.insert(("transitive-init-target", importee_idx))
            {
              continue;
            }
            obligations.push(self.strict_execution_order_init_obligation(
              "transitive-init-target",
              importer_idx,
              importee_idx,
              false,
            ));
          }
        }
      }
    }

    obligations
  }

  fn strict_execution_order_init_obligation(
    &self,
    kind: &'static str,
    importer_idx: ModuleIdx,
    importee_idx: ModuleIdx,
    awaited: bool,
  ) -> action::StrictExecutionOrderInitObligation {
    action::StrictExecutionOrderInitObligation {
      kind,
      importer_id: self.module_id(importer_idx),
      importee_id: self.module_id(importee_idx),
      awaited,
      importer_tla_tainted: self.link_output.metas[importer_idx].is_tla_or_contains_tla_dependency,
      importee_tla_tainted: self.link_output.metas[importee_idx].is_tla_or_contains_tla_dependency,
    }
  }

  fn wrapper_is_included(&self, module_idx: ModuleIdx) -> bool {
    let meta = &self.link_output.metas[module_idx];
    meta.wrapper_stmt_info.is_some_and(|stmt_idx| meta.stmt_info_included.has_bit(stmt_idx))
  }

  fn wrapper_is_reachable_from_module_chunk(
    &self,
    chunk_graph: &ChunkGraph,
    importer_idx: ModuleIdx,
    wrapper_ref: SymbolRef,
  ) -> bool {
    let Some(importer_chunk_idx) = chunk_graph.module_to_chunk[importer_idx] else {
      return false;
    };
    let canonical_wrapper_ref = self.link_output.symbol_db.canonical_ref_for(wrapper_ref);
    let Some(wrapper_chunk_idx) = self.link_output.symbol_db.get(canonical_wrapper_ref).chunk_idx
    else {
      return false;
    };
    wrapper_chunk_idx == importer_chunk_idx
      || chunk_graph.chunk_table[importer_chunk_idx]
        .imports_from_other_chunks
        .get(&wrapper_chunk_idx)
        .is_some_and(|items| {
          items.iter().any(|item| {
            self.link_output.symbol_db.canonical_ref_for(item.import_ref) == canonical_wrapper_ref
          })
        })
  }

  fn module_id(&self, module_idx: ModuleIdx) -> String {
    self.link_output.module_table[module_idx].id().to_string()
  }

  fn module_ids(&self, module_indices: &[ModuleIdx]) -> Vec<String> {
    module_indices.iter().map(|&module_idx| self.module_id(module_idx)).collect()
  }
}

fn wrap_kind_name(wrap_kind: WrapKind) -> &'static str {
  match wrap_kind {
    WrapKind::None => "none",
    WrapKind::Cjs => "cjs",
    WrapKind::Esm => "esm",
  }
}

fn is_rendered_chunk(chunk_graph: &ChunkGraph, chunk_idx: ChunkIdx) -> bool {
  !chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
}

pub(super) fn strict_execution_order_trace_requested(devtools_enabled: bool) -> bool {
  strict_execution_order_trace_enabled(
    devtools_enabled,
    std::env::var(STRICT_EXECUTION_ORDER_TRACE_ENV).is_ok_and(|value| value == "1"),
  )
}

fn strict_execution_order_trace_enabled(devtools_enabled: bool, explicit_opt_in: bool) -> bool {
  devtools_enabled && explicit_opt_in
}

#[cfg(test)]
mod tests {
  use rolldown_common::PostChunkOptimizationOperation;

  use super::*;

  #[test]
  fn post_optimization_chunks_are_not_rendered() {
    let mut chunk_graph = ChunkGraph::new(0);
    let chunk_idx = ChunkIdx::from_raw(0);

    assert!(is_rendered_chunk(&chunk_graph, chunk_idx));
    chunk_graph
      .post_chunk_optimization_operations
      .insert(chunk_idx, PostChunkOptimizationOperation::RemovedWithPreservedExports);
    assert!(!is_rendered_chunk(&chunk_graph, chunk_idx));
    chunk_graph
      .post_chunk_optimization_operations
      .insert(chunk_idx, PostChunkOptimizationOperation::Removed);
    assert!(!is_rendered_chunk(&chunk_graph, chunk_idx));
  }

  #[test]
  fn strict_trace_requires_devtools_and_explicit_opt_in() {
    assert!(!strict_execution_order_trace_enabled(false, false));
    assert!(!strict_execution_order_trace_enabled(false, true));
    assert!(!strict_execution_order_trace_enabled(true, false));
    assert!(strict_execution_order_trace_enabled(true, true));
  }
}
