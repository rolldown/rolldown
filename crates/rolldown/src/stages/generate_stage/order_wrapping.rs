use crate::{
  chunk_graph::ChunkGraph,
  stages::link_stage::{
    IncludeContext, SymbolIncludeReason, compute_body_demand_keys, create_wrapper,
    include_runtime_symbol, include_symbol,
  },
  types::linking_metadata::{
    included_info_to_linking_metadata_vec, linking_metadata_vec_to_included_info,
  },
};
use itertools::Itertools;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleIdx,
  PostChunkOptimizationOperation, RuntimeHelper, StmtEvalFlags, StmtInfoIdx, StmtInfoMeta,
  SymbolOrMemberExprRef, SymbolRef, UsedSymbolRefsBuilder, WrapKind,
};
use rolldown_utils::IndexBitSet;

use super::{
  GenerateStage,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  order_analysis::OrderWrapPlan,
  order_wrap_state::{OrderImportKey, OrderImportOverlay, OrderWrapState},
};

impl GenerateStage<'_> {
  pub(super) fn apply_order_wraps(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    plan: &OrderWrapPlan,
    used_symbol_refs: &mut UsedSymbolRefsBuilder,
    order_state: &mut OrderWrapState,
  ) {
    if plan.is_empty() {
      return;
    }

    let mut runtime_helpers = RuntimeHelper::default();
    for module_idx in
      plan.modules().sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order())
    {
      if !matches!(self.link_output.metas[module_idx].wrap_kind(), WrapKind::None) {
        continue;
      }
      let module = self.link_output.module_table[module_idx]
        .as_normal()
        .expect("order wrap only applies to normal modules");
      self.link_output.metas[module_idx].override_wrap_kind(WrapKind::Esm);
      self.link_output.metas[module_idx].hoist_esm_wrapper = true;
      create_wrapper(
        module,
        &mut self.link_output.stmt_infos[module_idx],
        &mut self.link_output.metas[module_idx],
        &mut self.link_output.symbol_db,
        &self.link_output.runtime,
        self.options,
      );
      order_state.record_legacy_order_wrapper(
        module_idx,
        self.link_output.metas[module_idx]
          .wrapper_ref
          .expect("legacy order wrapper should have a wrapper ref"),
      );
      self.ensure_order_wrap_stmt_inclusion_capacity(module_idx);
      runtime_helpers.insert(self.esm_runtime_helper());
      self.add_order_wrap_entry_reference(module_idx);
    }

    runtime_helpers |= self.reregister_order_wrap_imports(plan, order_state);
    self.include_order_wrap_symbols(plan, runtime_helpers, used_symbol_refs);
    self.place_order_wrap_modules(chunk_graph, plan);
    self.create_order_wrap_entry_facades(chunk_graph, plan);
    self.restore_order_wrap_dynamic_entry_facades(chunk_graph, plan);
    self.ensure_runtime_module_for_order_wraps(chunk_graph);
    chunk_graph.sort_chunk_modules(self.link_output, self.options);
    self.renumber_live_chunks(chunk_graph);
  }

  fn add_order_wrap_entry_reference(&mut self, module_idx: ModuleIdx) {
    if !self.link_output.entries.contains_key(&module_idx) {
      return;
    }
    let Some(wrapper_ref) = self.link_output.metas[module_idx].wrapper_ref else {
      return;
    };
    let referenced =
      &mut self.link_output.metas[module_idx].referenced_symbols_by_entry_point_chunk;
    if !referenced.iter().any(|(symbol_ref, _)| *symbol_ref == wrapper_ref) {
      referenced.push((wrapper_ref, false));
    }
  }

  fn ensure_order_wrap_stmt_inclusion_capacity(&mut self, module_idx: ModuleIdx) {
    let stmt_count = self.link_output.stmt_infos[module_idx].len();
    let old_included = self.link_output.metas[module_idx].stmt_info_included.clone();
    let mut included = IndexBitSet::new(stmt_count);
    for stmt_info_idx in old_included.index_of_one() {
      if stmt_info_idx.index() < stmt_count {
        included.set_bit(stmt_info_idx);
      }
    }
    self.link_output.metas[module_idx].stmt_info_included = included;
  }

  fn reregister_order_wrap_imports(
    &mut self,
    plan: &OrderWrapPlan,
    order_state: &mut OrderWrapState,
  ) -> RuntimeHelper {
    let mut runtime_helpers = RuntimeHelper::default();
    let module_indices = self
      .link_output
      .module_table
      .modules
      .iter_enumerated()
      .filter_map(|(idx, module)| module.as_normal().map(|_| idx))
      .collect_vec();

    for importer_idx in module_indices {
      let execution_dependencies = &self.link_output.metas[importer_idx].execution_dependencies;
      let affected_stmts = self.link_output.stmt_infos[importer_idx]
        .iter_enumerated()
        .filter_map(|(stmt_info_idx, stmt_info)| {
          let rec_ids = stmt_info
            .import_records
            .iter()
            .copied()
            .filter(|rec_idx| {
              self.link_output.module_table[importer_idx]
                .as_normal()
                .and_then(|importer| importer.import_records[*rec_idx].resolved_module)
                .is_some_and(|importee_idx| {
                  plan.contains(&importee_idx) && execution_dependencies.contains(&importee_idx)
                })
            })
            .collect_vec();
          (!rec_ids.is_empty()).then_some((stmt_info_idx, rec_ids))
        })
        .collect_vec();

      for (stmt_info_idx, rec_ids) in affected_stmts {
        for rec_idx in rec_ids {
          runtime_helpers |= self.reregister_order_wrap_import_record(
            importer_idx,
            stmt_info_idx,
            rec_idx,
            order_state,
          );
        }
      }
    }

    runtime_helpers
  }

  fn reregister_order_wrap_import_record(
    &mut self,
    importer_idx: ModuleIdx,
    stmt_info_idx: StmtInfoIdx,
    rec_idx: ImportRecordIdx,
    order_state: &mut OrderWrapState,
  ) -> RuntimeHelper {
    let Some(importer) = self.link_output.module_table[importer_idx].as_normal() else {
      return RuntimeHelper::default();
    };
    let rec = &importer.import_records[rec_idx];
    let Some(importee_idx) = rec.resolved_module else {
      return RuntimeHelper::default();
    };
    let Some(importee) = self.link_output.module_table[importee_idx].as_normal() else {
      return RuntimeHelper::default();
    };
    if !matches!(self.link_output.metas[importee_idx].wrap_kind(), WrapKind::Esm) {
      return RuntimeHelper::default();
    }

    let mut runtime_helpers = RuntimeHelper::default();
    let wrapper_ref =
      self.link_output.metas[importee_idx].wrapper_ref.expect("wrapped module has wrapper ref");
    let importee_namespace_ref = importee.namespace_object_ref;
    let importer_namespace_ref = importer.namespace_object_ref;
    let importee_has_dynamic_exports = self.link_output.metas[importee_idx].has_dynamic_exports;
    let importee_has_side_effects = importee.side_effects.has_side_effects();
    let overlay = OrderImportOverlay::from_import_record(
      rec.kind,
      rec.meta,
      wrapper_ref,
      importer_namespace_ref,
      importee_namespace_ref,
      importee_has_dynamic_exports,
      true,
      self.options.code_splitting.is_disabled(),
    );
    let overlay_runtime_helpers =
      overlay.as_ref().map_or_else(RuntimeHelper::default, |overlay| overlay.runtime_helpers);
    if let Some(overlay) = overlay {
      order_state.insert_import_overlay(
        OrderImportKey { importer: importer_idx, statement: stmt_info_idx, record: rec_idx },
        overlay,
        importer_namespace_ref,
        importee_namespace_ref,
      );
    }

    // TODO(strict-execution-order): Delete this compatibility write once finalization consumes
    // overlays. The lowering API must then borrow statements and linking metadata immutably so
    // generate-stage order lowering cannot reopen user-code liveness.
    let stmt_info = self.link_output.stmt_infos[importer_idx].get_mut(stmt_info_idx);
    match rec.kind {
      ImportKind::Import => {
        let is_reexport_all = rec.meta.contains(ImportRecordMeta::IsExportStar);
        stmt_info
          .eval_flags
          .set(StmtEvalFlags::UnknownSideEffect, is_reexport_all || importee_has_side_effects);
        push_symbol_once(&mut stmt_info.referenced_symbols, wrapper_ref);

        if is_reexport_all && importee_has_dynamic_exports {
          stmt_info.meta.insert(StmtInfoMeta::ReExportDynamicExports);
          push_symbol_once(&mut stmt_info.referenced_symbols, importer_namespace_ref);
          push_symbol_once(&mut stmt_info.referenced_symbols, importee_namespace_ref);
          runtime_helpers.insert(RuntimeHelper::ReExport);
        }
      }
      ImportKind::Require => {
        push_symbol_once(&mut stmt_info.referenced_symbols, wrapper_ref);
        push_symbol_once(&mut stmt_info.referenced_symbols, importee_namespace_ref);
        if !rec.meta.contains(ImportRecordMeta::IsRequireUnused) {
          runtime_helpers.insert(RuntimeHelper::ToCommonJs);
        }
      }
      ImportKind::DynamicImport if self.options.code_splitting.is_disabled() => {
        push_symbol_once(&mut stmt_info.referenced_symbols, wrapper_ref);
        push_symbol_once(&mut stmt_info.referenced_symbols, importee_namespace_ref);
      }
      ImportKind::DynamicImport
      | ImportKind::AtImport
      | ImportKind::UrlImport
      | ImportKind::NewUrl
      | ImportKind::HotAccept => {}
    }

    debug_assert_eq!(runtime_helpers, overlay_runtime_helpers);
    overlay_runtime_helpers
  }

  fn include_order_wrap_symbols(
    &mut self,
    plan: &OrderWrapPlan,
    runtime_helpers: RuntimeHelper,
    used_symbol_refs: &mut UsedSymbolRefsBuilder,
  ) {
    let (mut stmt_info_included_vec, module_included_vec, mut module_namespace_reason_vec) =
      linking_metadata_vec_to_included_info(&mut self.link_output.metas);
    let body_demand_keys = compute_body_demand_keys(
      &self.link_output.module_table.modules,
      &self.link_output.stmt_infos,
      &self.link_output.symbol_db,
      self.options.treeshake.is_some(),
      &self.link_output.user_defined_entry_modules,
    );

    let mut module_included_vec = module_included_vec;
    {
      let mut context = IncludeContext::new(
        &self.link_output.module_table.modules,
        &self.link_output.stmt_infos,
        &self.link_output.symbol_db,
        &mut stmt_info_included_vec,
        &mut module_included_vec,
        self.link_output.runtime.id(),
        &self.link_output.metas,
        used_symbol_refs,
        &mut self.link_output.used_external_symbols,
        &self.link_output.global_constant_symbol_map,
        self.options,
        &self.link_output.normal_symbol_exports_chain_map,
        &mut module_namespace_reason_vec,
        &self.link_output.user_defined_entry_modules,
        &body_demand_keys,
      );

      for module_idx in plan.modules() {
        if let Some(wrapper_ref) = self.link_output.metas[module_idx].wrapper_ref {
          include_symbol(&mut context, wrapper_ref, SymbolIncludeReason::Normal);
        }
      }
      include_runtime_symbol(&mut context, &self.link_output.runtime, runtime_helpers);
    }

    included_info_to_linking_metadata_vec(
      &mut self.link_output.metas,
      stmt_info_included_vec,
      &module_included_vec,
      &module_namespace_reason_vec,
    );
  }

  fn place_order_wrap_modules(&self, chunk_graph: &mut ChunkGraph, plan: &OrderWrapPlan) {
    let sorted_order_wraps = plan
      .modules()
      .sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order())
      .collect_vec();

    loop {
      let mut changed = false;
      for module_idx in &sorted_order_wraps {
        if chunk_graph.module_to_chunk[*module_idx].is_some() {
          continue;
        }
        if let Some(chunk_idx) = self.preferred_order_wrap_chunk(*module_idx, chunk_graph) {
          chunk_graph.add_module_to_chunk(
            *module_idx,
            chunk_idx,
            self.link_output.metas[*module_idx].depended_runtime_helper,
          );
          changed = true;
        }
      }

      if !changed {
        break;
      }
    }

    let Some(fallback_chunk_idx) = self.first_live_chunk(chunk_graph) else {
      return;
    };
    for module_idx in sorted_order_wraps {
      if chunk_graph.module_to_chunk[module_idx].is_some() {
        continue;
      }
      chunk_graph.add_module_to_chunk(
        module_idx,
        fallback_chunk_idx,
        self.link_output.metas[module_idx].depended_runtime_helper,
      );
    }
  }

  fn create_order_wrap_entry_facades(&self, chunk_graph: &mut ChunkGraph, plan: &OrderWrapPlan) {
    if self.options.code_splitting.is_disabled() {
      return;
    }

    let mut entries_to_split = plan
      .modules()
      .filter(|module_idx| self.link_output.entries.contains_key(module_idx))
      .collect_vec();
    for module_idx in plan.modules() {
      let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
        continue;
      };
      entries_to_split.extend(module.import_records.iter().filter_map(|rec| {
        if rec.kind != ImportKind::Import {
          return None;
        }
        let importee_idx = rec.resolved_module?;
        (self.link_output.entries.contains_key(&importee_idx)
          && self.link_output.metas[module_idx].execution_dependencies.contains(&importee_idx)
          && matches!(self.link_output.metas[importee_idx].original_wrap_kind(), WrapKind::Cjs))
        .then_some(importee_idx)
      }));
    }
    entries_to_split.sort_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order());
    entries_to_split.dedup();

    for entry_module_idx in entries_to_split {
      let Some(entry_chunk_idx) =
        chunk_graph.entry_module_to_entry_chunk.get(&entry_module_idx).copied()
      else {
        continue;
      };
      if matches!(
        chunk_graph.post_chunk_optimization_operations.get(&entry_chunk_idx),
        Some(PostChunkOptimizationOperation::Removed)
      ) {
        continue;
      }

      let Some((meta, bit, name, file_name, bits, input_base, preserve_entry_signature)) = ({
        let entry_chunk = &mut chunk_graph.chunk_table[entry_chunk_idx];
        match entry_chunk.kind {
          ChunkKind::EntryPoint { meta, bit, module }
            if module == entry_module_idx && !entry_chunk.modules.is_empty() =>
          {
            let bits = entry_chunk.bits.clone();
            let input_base = entry_chunk.input_base.clone();
            let name = entry_chunk.name.take();
            let file_name = entry_chunk.file_name.take();
            let preserve_entry_signature = entry_chunk.preserve_entry_signature.take();
            entry_chunk.kind = ChunkKind::Common;
            entry_chunk.add_creation_reason(
              ChunkCreationReason::CommonChunk { bits: &bits, link_output: self.link_output },
              self.options,
            );
            Some((meta, bit, name, file_name, bits, input_base, preserve_entry_signature))
          }
          ChunkKind::EntryPoint { .. } | ChunkKind::Common => None,
        }
      }) else {
        continue;
      };

      let mut facade_chunk = Chunk::new(
        name,
        file_name,
        bits,
        vec![],
        ChunkKind::EntryPoint { meta, bit, module: entry_module_idx },
        input_base,
        preserve_entry_signature,
      );
      let entry_module = &self.link_output.module_table[entry_module_idx];
      facade_chunk.add_creation_reason(
        ChunkCreationReason::Entry {
          is_user_defined_entry: meta.contains(ChunkMeta::UserDefinedEntry),
          entry_module_id: entry_module.stable_id(),
          name: self
            .link_output
            .entries
            .get(&entry_module_idx)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.name.as_ref()),
        },
        self.options,
      );
      let facade_chunk_idx = chunk_graph.add_chunk(facade_chunk);
      chunk_graph.entry_module_to_entry_chunk.insert(entry_module_idx, facade_chunk_idx);
      if let Some(reference_ids) = chunk_graph.chunk_idx_to_reference_ids.remove(&entry_chunk_idx) {
        chunk_graph.chunk_idx_to_reference_ids.insert(facade_chunk_idx, reference_ids);
      }
    }
  }

  fn restore_order_wrap_dynamic_entry_facades(
    &self,
    chunk_graph: &mut ChunkGraph,
    plan: &OrderWrapPlan,
  ) {
    if self.options.code_splitting.is_disabled() {
      return;
    }

    let dynamic_entries_to_restore = plan
      .modules()
      .filter(|module_idx| {
        self
          .link_output
          .entries
          .get(module_idx)
          .is_some_and(|entries| entries.iter().any(|entry| entry.kind.is_dynamic_import()))
      })
      .sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order())
      .collect_vec();

    for entry_module_idx in dynamic_entries_to_restore {
      let Some(facade_chunk_idx) =
        chunk_graph.chunk_table.iter_enumerated().find_map(|(chunk_idx, chunk)| match chunk.kind {
          ChunkKind::EntryPoint { meta, module, .. }
            if module == entry_module_idx
              && chunk.modules.is_empty()
              && meta.contains(ChunkMeta::DynamicImported)
              && matches!(
                chunk_graph.post_chunk_optimization_operations.get(&chunk_idx),
                Some(
                  PostChunkOptimizationOperation::Removed
                    | PostChunkOptimizationOperation::RemovedWithPreservedExports
                )
              ) =>
          {
            Some(chunk_idx)
          }
          ChunkKind::EntryPoint { .. } | ChunkKind::Common => None,
        })
      else {
        continue;
      };

      if let Some(current_chunk_idx) =
        chunk_graph.entry_module_to_entry_chunk.insert(entry_module_idx, facade_chunk_idx)
      {
        let should_remove_key = if let Some(set) =
          chunk_graph.common_chunk_exported_facade_chunk_namespace.get_mut(&current_chunk_idx)
        {
          set.remove(&entry_module_idx);
          set.is_empty()
        } else {
          false
        };
        if should_remove_key {
          chunk_graph.common_chunk_exported_facade_chunk_namespace.remove(&current_chunk_idx);
        }
      }

      chunk_graph.post_chunk_optimization_operations.remove(&facade_chunk_idx);
    }
  }

  fn preferred_order_wrap_chunk(
    &self,
    module_idx: ModuleIdx,
    chunk_graph: &ChunkGraph,
  ) -> Option<ChunkIdx> {
    let module = self.link_output.module_table[module_idx].as_normal()?;

    module
      .import_records
      .iter()
      .filter(|rec| rec.kind == ImportKind::Import)
      .filter_map(|rec| rec.resolved_module)
      .filter_map(|importee_idx| chunk_graph.module_to_chunk[importee_idx])
      .find(|chunk_idx| is_live_chunk(chunk_graph, *chunk_idx))
      .or_else(|| {
        module
          .importers_idx
          .iter()
          .copied()
          .sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order())
          .filter_map(|importer_idx| chunk_graph.module_to_chunk[importer_idx])
          .find(|chunk_idx| is_live_chunk(chunk_graph, *chunk_idx))
      })
  }

  fn ensure_runtime_module_for_order_wraps(&mut self, chunk_graph: &mut ChunkGraph) {
    let runtime_idx = self.link_output.runtime.id();
    if let Some(runtime_chunk_idx) = chunk_graph.module_to_chunk[runtime_idx] {
      if self.options.code_splitting.is_disabled() {
        return;
      }
      let runtime_chunk = &chunk_graph.chunk_table[runtime_chunk_idx];
      if runtime_chunk.modules.len() == 1 {
        self.clear_module_symbol_chunk_indices(runtime_idx);
        return;
      }
      let mut bits = runtime_chunk.bits.clone();
      for chunk_idx in self.live_chunks(chunk_graph) {
        bits.union(&chunk_graph.chunk_table[chunk_idx].bits);
      }
      let input_base = runtime_chunk.input_base.clone();
      chunk_graph.chunk_table[runtime_chunk_idx]
        .modules
        .retain(|module_idx| *module_idx != runtime_idx);
      self.update_chunk_runtime_helpers_after_module_removal(
        chunk_graph,
        runtime_chunk_idx,
        runtime_idx,
      );
      let mut new_runtime_chunk = Chunk::new(
        Some("rolldown-runtime".into()),
        None,
        bits,
        vec![],
        ChunkKind::Common,
        input_base,
        None,
      );
      let runtime_chunk_bits = new_runtime_chunk.bits.clone();
      new_runtime_chunk.add_creation_reason(
        ChunkCreationReason::CommonChunk {
          bits: &runtime_chunk_bits,
          link_output: self.link_output,
        },
        self.options,
      );
      let new_runtime_chunk_idx = chunk_graph.add_chunk(new_runtime_chunk);
      chunk_graph.add_module_to_chunk(
        runtime_idx,
        new_runtime_chunk_idx,
        self.link_output.metas[runtime_idx].depended_runtime_helper,
      );
      self.clear_module_symbol_chunk_indices(runtime_idx);
      return;
    }

    let live_chunk_indices = self.live_chunks(chunk_graph);
    let Some(first_chunk_idx) = live_chunk_indices.first().copied() else {
      return;
    };

    if self.options.code_splitting.is_disabled() || live_chunk_indices.len() == 1 {
      let chunk = &mut chunk_graph.chunk_table[first_chunk_idx];
      chunk.modules.insert(0, runtime_idx);
      chunk_graph.module_to_chunk[runtime_idx] = Some(first_chunk_idx);
      self.clear_module_symbol_chunk_indices(runtime_idx);
      return;
    }

    let mut bits = chunk_graph.chunk_table[first_chunk_idx].bits.clone();
    for chunk_idx in live_chunk_indices.iter().copied().skip(1) {
      bits.union(&chunk_graph.chunk_table[chunk_idx].bits);
    }

    let input_base = chunk_graph.chunk_table[first_chunk_idx].input_base.clone();
    let mut runtime_chunk = Chunk::new(
      Some("rolldown-runtime".into()),
      None,
      bits,
      vec![],
      ChunkKind::Common,
      input_base,
      None,
    );
    let runtime_chunk_bits = runtime_chunk.bits.clone();
    runtime_chunk.add_creation_reason(
      ChunkCreationReason::CommonChunk { bits: &runtime_chunk_bits, link_output: self.link_output },
      self.options,
    );
    let runtime_chunk_idx = chunk_graph.add_chunk(runtime_chunk);
    chunk_graph.add_module_to_chunk(
      runtime_idx,
      runtime_chunk_idx,
      self.link_output.metas[runtime_idx].depended_runtime_helper,
    );
    self.clear_module_symbol_chunk_indices(runtime_idx);
  }

  fn update_chunk_runtime_helpers_after_module_removal(
    &self,
    chunk_graph: &mut ChunkGraph,
    chunk_idx: ChunkIdx,
    removed_module_idx: ModuleIdx,
  ) {
    let mut helpers = chunk_graph.chunk_table[chunk_idx].depended_runtime_helper;
    helpers.remove(self.link_output.metas[removed_module_idx].depended_runtime_helper);
    helpers.insert(
      chunk_graph.chunk_table[chunk_idx]
        .modules
        .iter()
        .fold(RuntimeHelper::default(), |helpers, module_idx| {
          helpers | self.link_output.metas[*module_idx].depended_runtime_helper
        }),
    );
    chunk_graph.chunk_table[chunk_idx].depended_runtime_helper = helpers;
  }

  fn clear_module_symbol_chunk_indices(&mut self, module_idx: ModuleIdx) {
    let Some(local_db) = self.link_output.symbol_db[module_idx].as_mut() else {
      return;
    };
    for symbol_data in &mut local_db.classic_data {
      symbol_data.chunk_idx = None;
    }
  }

  fn live_chunks(&self, chunk_graph: &ChunkGraph) -> Vec<ChunkIdx> {
    chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter_map(|(chunk_idx, _)| is_live_chunk(chunk_graph, chunk_idx).then_some(chunk_idx))
      .collect_vec()
  }

  fn first_live_chunk(&self, chunk_graph: &ChunkGraph) -> Option<ChunkIdx> {
    self.live_chunks(chunk_graph).first().copied()
  }

  fn renumber_live_chunks(&self, chunk_graph: &mut ChunkGraph) {
    let live_chunks = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(chunk_idx, _)| {
        !matches!(
          chunk_graph.post_chunk_optimization_operations.get(chunk_idx),
          Some(PostChunkOptimizationOperation::Removed)
        )
      })
      .sorted_by_key(|(chunk_idx, chunk)| (chunk.exec_order, chunk_idx.raw()))
      .map(|(chunk_idx, _)| chunk_idx)
      .collect_vec();

    for (exec_order, chunk_idx) in live_chunks.iter().copied().enumerate() {
      chunk_graph.chunk_table[chunk_idx].exec_order =
        exec_order.try_into().expect("Too many chunks, u32 overflowed.");
    }

    chunk_graph.sorted_chunk_idx_vec = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(chunk_idx, _)| {
        !matches!(
          chunk_graph.post_chunk_optimization_operations.get(chunk_idx),
          Some(PostChunkOptimizationOperation::Removed)
        )
      })
      .sorted_unstable_by_key(|(index, chunk)| match &chunk.kind {
        ChunkKind::EntryPoint { meta, .. } if meta.contains(ChunkMeta::UserDefinedEntry) => {
          (0, index.raw())
        }
        _ => (1, chunk.exec_order),
      })
      .map(|(idx, _)| idx)
      .collect();
  }

  fn esm_runtime_helper(&self) -> RuntimeHelper {
    if self.options.profiler_names { RuntimeHelper::Esm } else { RuntimeHelper::EsmMin }
  }
}

fn push_symbol_once(referenced_symbols: &mut Vec<SymbolOrMemberExprRef>, symbol_ref: SymbolRef) {
  if !referenced_symbols.iter().any(|item| item.symbol_ref() == &symbol_ref) {
    referenced_symbols.push(symbol_ref.into());
  }
}

fn is_live_chunk(chunk_graph: &ChunkGraph, chunk_idx: ChunkIdx) -> bool {
  !matches!(
    chunk_graph.post_chunk_optimization_operations.get(&chunk_idx),
    Some(PostChunkOptimizationOperation::Removed)
  )
}
