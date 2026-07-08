use crate::{
  chunk_graph::ChunkGraph, type_alias::IndexStmtInfos, types::linking_metadata::LinkingMetadataVec,
};
use itertools::Itertools;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ImportKind, ImportRecordIdx, ImportRecordMeta,
  IndexModules, ModuleIdx, PostChunkOptimizationOperation, RuntimeHelper, StmtInfoIdx, SymbolRef,
  SymbolRefDb, UsedSymbolRefsBuilder, WrapKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  GenerateStage,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  order_analysis::OrderWrapPlan,
  order_wrap_state::{OrderImportKey, OrderImportOverlay, OrderWrapState},
};

struct OrderLoweringInput<'a> {
  plan: &'a OrderWrapPlan,
  modules: &'a IndexModules,
  linking: &'a LinkingMetadataVec,
  statements: &'a IndexStmtInfos,
  export_chains: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
  star_reexport_records_by_imported_symbol:
    &'a FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, ImportRecordIdx)>>>,
  used_symbols: &'a UsedSymbolRefsBuilder,
}

struct OrderLoweringOutput<'a> {
  symbols: &'a mut SymbolRefDb,
  state: &'a mut OrderWrapState,
}

struct FrozenReexportUsage {
  root_paths: FxHashMap<(ModuleIdx, ImportRecordIdx), Vec<(ModuleIdx, ImportRecordIdx)>>,
  nested_records: FxHashSet<(ModuleIdx, ImportRecordIdx)>,
  consumed_facades: FxHashSet<SymbolRef>,
}

impl GenerateStage<'_> {
  pub(super) fn apply_order_wraps(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    plan: &OrderWrapPlan,
    used_symbol_refs: &UsedSymbolRefsBuilder,
    order_state: &mut OrderWrapState,
  ) {
    if plan.is_empty() {
      return;
    }

    let runtime_helper = self.esm_runtime_helper();
    let code_splitting_disabled = self.options.code_splitting.is_disabled();
    let input = OrderLoweringInput {
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
    let mut output =
      OrderLoweringOutput { symbols: &mut self.link_output.symbol_db, state: order_state };
    lower_order_state(&input, &mut output, runtime_helper, code_splitting_disabled);
    let runtime_idx = self.link_output.runtime.id();
    order_state.compute_runtime_symbol_closure(
      &self.link_output.runtime,
      &self.link_output.stmt_infos[runtime_idx],
      &self.link_output.symbol_db,
    );
    self.place_order_wrap_modules(chunk_graph, plan, order_state);
    self.create_strict_execution_order_entry_facades(chunk_graph, Some(plan));
    self.restore_order_wrap_dynamic_entry_facades(chunk_graph, plan);
    self.ensure_runtime_module_for_order_wraps(chunk_graph);
    chunk_graph.sort_chunk_modules(self.link_output, self.options);
    self.renumber_live_chunks(chunk_graph);
  }

  fn place_order_wrap_modules(
    &self,
    chunk_graph: &mut ChunkGraph,
    plan: &OrderWrapPlan,
    order_state: &mut OrderWrapState,
  ) {
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

    let order_wrapped_modules =
      plan.modules().filter(|module_idx| order_state.has_order_wrapper(*module_idx)).collect_vec();
    for module_idx in order_wrapped_modules {
      let chunk_idx =
        chunk_graph.module_to_chunk[module_idx].expect("order-wrapped module should have a chunk");
      order_state.assign_order_wrapper_chunk(module_idx, chunk_idx);
    }
  }

  pub(super) fn create_strict_execution_order_entry_facades(
    &self,
    chunk_graph: &mut ChunkGraph,
    plan: Option<&OrderWrapPlan>,
  ) -> bool {
    if !self.options.is_strict_execution_order_enabled()
      || self.options.code_splitting.is_disabled()
    {
      return false;
    }

    let mut entries_to_split = plan
      .into_iter()
      .flat_map(OrderWrapPlan::modules)
      .filter(|module_idx| self.link_output.entries.contains_key(module_idx))
      .collect_vec();
    for module in
      self.link_output.module_table.modules.iter().filter_map(|module| module.as_normal())
    {
      let meta = &self.link_output.metas[module.idx];
      if !meta.is_included {
        continue;
      }
      entries_to_split.extend(module.import_records.iter().filter_map(|rec| {
        if !matches!(rec.kind, ImportKind::Import | ImportKind::Require) {
          return None;
        }
        let importee_idx = rec.resolved_module?;
        (self.link_output.entries.contains_key(&importee_idx)
          && meta.execution_dependencies.contains(&importee_idx)
          && !matches!(self.link_output.metas[importee_idx].wrap_kind(), WrapKind::None))
        .then_some(importee_idx)
      }));
    }
    entries_to_split.sort_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order());
    entries_to_split.dedup();

    let mut changed = false;
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
      changed = true;
    }
    changed
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

  pub(super) fn renumber_live_chunks(&self, chunk_graph: &mut ChunkGraph) {
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

fn lower_order_state(
  input: &OrderLoweringInput<'_>,
  output: &mut OrderLoweringOutput<'_>,
  runtime_helper: RuntimeHelper,
  code_splitting_disabled: bool,
) {
  let reexport_usage = collect_frozen_reexport_usage(input);
  output.state.set_nested_reexport_records(reexport_usage.nested_records.clone());
  for module_idx in
    input.plan.modules().sorted_unstable_by_key(|idx| input.modules[*idx].exec_order())
  {
    if !matches!(input.linking[module_idx].wrap_kind(), WrapKind::None) {
      continue;
    }
    let module =
      input.modules[module_idx].as_normal().expect("order wrap only applies to normal modules");
    let wrapper_ref = output
      .symbols
      .create_facade_root_symbol_ref(module_idx, &format!("init_{}", module.repr_name));
    output.state.insert_order_wrapper(module_idx, wrapper_ref, runtime_helper);
  }

  for (importer_idx, module) in input.modules.iter_enumerated() {
    let Some(importer) = module.as_normal() else {
      continue;
    };
    let execution_dependencies = &input.linking[importer_idx].execution_dependencies;
    for (stmt_info_idx, stmt_info) in input.statements[importer_idx].iter_enumerated() {
      for &rec_idx in &stmt_info.import_records {
        let rec = &importer.import_records[rec_idx];
        let Some(importee_idx) = rec.resolved_module else {
          continue;
        };
        let direct_target_is_planned = input.plan.contains(&importee_idx);
        let retained_reexport_path = retained_order_reexport_path(
          input,
          &reexport_usage,
          importer_idx,
          stmt_info_idx,
          rec_idx,
        );
        if !execution_dependencies.contains(&importee_idx) && retained_reexport_path.is_none() {
          continue;
        }
        let Some(importee) = input.modules[importee_idx].as_normal() else {
          continue;
        };
        if !direct_target_is_planned {
          if let Some(retained_reexport_path) = retained_reexport_path
            && static_import_reaches_plan(input, importee_idx)
          {
            output.state.insert_import_overlay(
              OrderImportKey { importer: importer_idx, statement: stmt_info_idx, record: rec_idx },
              OrderImportOverlay::transitive_reexport(retained_reexport_path),
              importer.namespace_object_ref,
              importee.namespace_object_ref,
            );
          }
          continue;
        }
        let Some(init_target) =
          output.state.esm_init_target(importee_idx, &input.linking[importee_idx])
        else {
          continue;
        };
        let mut overlay = OrderImportOverlay::from_import_record(
          rec.kind,
          rec.meta,
          init_target.wrapper_ref,
          importer.namespace_object_ref,
          importee.namespace_object_ref,
          input.linking[importee_idx].has_dynamic_exports,
          execution_dependencies.contains(&importee_idx),
          code_splitting_disabled,
        );
        if let Some(overlay) = &mut overlay
          && let Some(retained_reexport_path) = retained_reexport_path
        {
          overlay.retained_reexport_path = retained_reexport_path;
        }
        if let Some(overlay) = overlay {
          output.state.insert_import_overlay(
            OrderImportKey { importer: importer_idx, statement: stmt_info_idx, record: rec_idx },
            overlay,
            importer.namespace_object_ref,
            importee.namespace_object_ref,
          );
        }
      }
    }
  }
}

fn static_import_reaches_plan(input: &OrderLoweringInput<'_>, root: ModuleIdx) -> bool {
  let mut visited = rustc_hash::FxHashSet::default();
  let mut stack = vec![root];
  while let Some(module_idx) = stack.pop() {
    if !visited.insert(module_idx) {
      continue;
    }
    if input.plan.contains(&module_idx) {
      return true;
    }
    let Some(module) = input.modules[module_idx].as_normal() else {
      continue;
    };
    stack.extend(
      module
        .import_records
        .iter()
        .filter(|rec| rec.kind == ImportKind::Import)
        .filter_map(|rec| rec.resolved_module),
    );
  }
  false
}

fn collect_frozen_reexport_usage(input: &OrderLoweringInput<'_>) -> FrozenReexportUsage {
  let mut consumed_facades = FxHashSet::default();
  for (used_ref, chain) in input.export_chains {
    if input.used_symbols.contains(used_ref) {
      consumed_facades.extend(chain.iter().copied());
    }
  }

  let mut root_paths =
    FxHashMap::<(ModuleIdx, ImportRecordIdx), Vec<(ModuleIdx, ImportRecordIdx)>>::default();
  for (imported_as_ref, paths) in input.star_reexport_records_by_imported_symbol {
    for path in paths {
      let Some(root) = path.first().copied() else {
        continue;
      };
      let consumer_is_used = input.used_symbols.contains(imported_as_ref)
        || consumed_facades.contains(imported_as_ref)
        || input.linking[root.0]
          .referenced_symbols_by_entry_point_chunk
          .iter()
          .any(|(symbol_ref, _)| symbol_ref == imported_as_ref);
      if consumer_is_used {
        root_paths.entry(root).or_default().extend(path.iter().copied());
      }
    }
  }

  let mut nested_records = FxHashSet::default();
  for (root, path) in &mut root_paths {
    path.sort_unstable_by_key(|(module_idx, rec_idx)| (module_idx.index(), rec_idx.index()));
    path.dedup();
    nested_records.extend(path.iter().copied().filter(|record| record != root));
  }

  FrozenReexportUsage { root_paths, nested_records, consumed_facades }
}

fn retained_order_reexport_path(
  input: &OrderLoweringInput<'_>,
  reexport_usage: &FrozenReexportUsage,
  importer_idx: ModuleIdx,
  stmt_info_idx: StmtInfoIdx,
  rec_idx: ImportRecordIdx,
) -> Option<Vec<(ModuleIdx, ImportRecordIdx)>> {
  let importer = input.modules[importer_idx].as_normal()?;
  let rec = &importer.import_records[rec_idx];
  if !rec.meta.intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly) {
    return None;
  }
  let meta = &input.linking[importer_idx];
  if let Some(path) = reexport_usage.root_paths.get(&(importer_idx, rec_idx)) {
    return Some(path.clone());
  }
  if reexport_usage.nested_records.contains(&(importer_idx, rec_idx)) {
    return None;
  }
  if meta.stmt_info_included.has_bit(stmt_info_idx) {
    return Some(vec![]);
  }
  if rec.meta.contains(ImportRecordMeta::IsExportStar)
    && meta.namespace_included
    && rec
      .resolved_module
      .is_some_and(|importee_idx| input.linking[importee_idx].has_dynamic_exports)
  {
    return Some(vec![]);
  }

  let facade_is_retained = |facade_ref: SymbolRef| {
    input.used_symbols.contains(&facade_ref)
      || reexport_usage.consumed_facades.contains(&facade_ref)
  };
  (importer
    .named_imports
    .iter()
    .any(|(facade_ref, import)| import.record_idx == rec_idx && facade_is_retained(*facade_ref))
    || input.statements[importer_idx][stmt_info_idx]
      .declared_symbols
      .iter()
      .any(|declared| facade_is_retained(declared.inner())))
  .then_some(vec![])
}

fn is_live_chunk(chunk_graph: &ChunkGraph, chunk_idx: ChunkIdx) -> bool {
  !matches!(
    chunk_graph.post_chunk_optimization_operations.get(&chunk_idx),
    Some(PostChunkOptimizationOperation::Removed)
  )
}
