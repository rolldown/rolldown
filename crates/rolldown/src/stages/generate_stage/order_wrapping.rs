use crate::{
  chunk_graph::ChunkGraph,
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};
use itertools::Itertools;
use oxc::ast::ast::{Declaration, ExportDefaultDeclarationKind, Statement};
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ImportKind, ImportRecordIdx, ImportRecordMeta,
  IndexModules, ModuleIdx, OutputFormat, PostChunkOptimizationOperation, RuntimeHelper,
  StmtInfoIdx, SymbolRef, SymbolRefDb, UsedSymbolRefsBuilder, WrapKind,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::StatementExt;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  GenerateStage,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  chunk_optimizer::RuntimeMergeCascade,
  order_analysis::{OrderAnalysis, OrderWrapPlan},
  order_wrap_state::{OrderImportKey, OrderImportOverlay, OrderWrapState},
};

pub(super) struct OrderLoweringInput<'a> {
  pub(super) plan: &'a OrderWrapPlan,
  pub(super) modules: &'a IndexModules,
  pub(super) linking: &'a LinkingMetadataVec,
  pub(super) statements: &'a IndexStmtInfos,
  pub(super) asts: &'a IndexEcmaAst,
  pub(super) keep_names: bool,
  pub(super) export_chains: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub(super) star_reexport_records_by_imported_symbol:
    &'a FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, ImportRecordIdx)>>>,
  pub(super) used_symbols: &'a UsedSymbolRefsBuilder,
}

/// Whether an execution-order wrapper is only a routing waypoint for re-export initialization.
///
/// Unlike `init_is_noop`, this deliberately ignores import/re-export lowering glue: that glue is
/// consumer-dependent and retained leaf initialization must be routed from the consuming record,
/// not installed into a shared pure barrel wrapper. Local executable statements, generated missing
/// export assignments, `keepNames` calls, and unconditional execution dependencies make the
/// wrapper non-transparent.
pub(super) fn order_wrapper_is_reexport_transparent(
  meta: &LinkingMetadata,
  ast: Option<&EcmaAst>,
  keep_names: bool,
) -> bool {
  matches!(
    meta.concatenated_wrapped_module_kind,
    rolldown_common::ConcatenateWrappedModuleKind::None
  ) && meta.shimmed_missing_exports.is_empty()
    && meta.execution_dependencies.is_empty()
    && ast.is_some_and(|ast| {
      ast.program().body.iter().all(|stmt| statement_has_no_local_wrapper_body(stmt, keep_names))
    })
}

fn statement_has_no_local_wrapper_body(stmt: &Statement, keep_names: bool) -> bool {
  // Static import/re-export statements may lower to init forwarding or namespace glue, but that
  // work is routed per consumer for transparent wrappers and is not a module-local executable body.
  if stmt.is_module_declaration_with_source() {
    return true;
  }
  match stmt {
    Statement::FunctionDeclaration(_) => !keep_names,
    Statement::ExportDefaultDeclaration(export) => {
      matches!(export.declaration, ExportDefaultDeclarationKind::FunctionDeclaration(_))
        && !keep_names
    }
    Statement::ExportNamedDeclaration(export) => match &export.declaration {
      None => true,
      Some(Declaration::FunctionDeclaration(_)) => !keep_names,
      Some(_) => false,
    },
    _ => false,
  }
}

struct OrderLoweringOutput<'a> {
  symbols: &'a mut SymbolRefDb,
  state: &'a mut OrderWrapState,
}

pub(super) struct FrozenReexportUsage {
  root_paths: FxHashMap<(ModuleIdx, ImportRecordIdx), Vec<(ModuleIdx, ImportRecordIdx)>>,
  nested_records: FxHashSet<(ModuleIdx, ImportRecordIdx)>,
  consumed_facades: FxHashSet<SymbolRef>,
}

impl FrozenReexportUsage {
  pub(super) fn nested_records(&self) -> &FxHashSet<(ModuleIdx, ImportRecordIdx)> {
    &self.nested_records
  }

  pub(super) fn consumed_facades(&self) -> &FxHashSet<SymbolRef> {
    &self.consumed_facades
  }
}

impl GenerateStage<'_> {
  /// Returns whether the chunk topology changed.
  pub(super) fn apply_order_wraps(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    analysis: &OrderAnalysis,
    used_symbol_refs: &UsedSymbolRefsBuilder,
    order_state: &mut OrderWrapState,
  ) -> bool {
    let plan = &analysis.plan;
    if plan.is_empty() {
      // Entry-trigger facades are needed even with an empty plan: a pure interop graph can
      // still share one entry's chunk with another entry.
      if !self.create_order_wrap_entry_facades(chunk_graph, analysis) {
        return false;
      }
      chunk_graph.sort_chunk_modules(self.link_output, self.options);
      self.renumber_live_chunks(chunk_graph);
      return true;
    }

    let runtime_helper = self.esm_runtime_helper();
    let code_splitting_disabled = self.options.code_splitting.is_disabled();
    let input = OrderLoweringInput {
      plan,
      modules: &self.link_output.module_table.modules,
      linking: &self.link_output.metas,
      statements: &self.link_output.stmt_infos,
      asts: &self.ast_table,
      keep_names: self.options.keep_names,
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
    self.create_order_wrap_entry_facades(chunk_graph, analysis);
    self.restore_order_wrap_entry_facades(chunk_graph, plan);
    self.ensure_runtime_module_for_order_wraps(chunk_graph, order_state);
    chunk_graph.sort_chunk_modules(self.link_output, self.options);
    self.renumber_live_chunks(chunk_graph);
    true
  }

  fn place_order_wrap_modules(
    &self,
    chunk_graph: &ChunkGraph,
    plan: &OrderWrapPlan,
    order_state: &mut OrderWrapState,
  ) {
    // Plan members are included user modules, so chunk assignment already placed every one.
    // Sorted so synthetic-statement registration order (which feeds deconfliction naming)
    // stays deterministic.
    let order_wrapped_modules = plan
      .modules()
      .filter(|module_idx| order_state.has_order_wrapper(*module_idx))
      .sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order())
      .collect_vec();
    for module_idx in order_wrapped_modules {
      let chunk_idx =
        chunk_graph.module_to_chunk[module_idx].expect("order-wrapped module should have a chunk");
      order_state.assign_order_wrapper_chunk(module_idx, chunk_idx);
    }
  }

  fn create_order_wrap_entry_facades(
    &self,
    chunk_graph: &mut ChunkGraph,
    analysis: &OrderAnalysis,
  ) -> bool {
    if self.options.code_splitting.is_disabled() {
      return false;
    }
    let plan = &analysis.plan;

    let mut entries_to_split = plan
      .modules()
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

    // Move an interop entry trigger to a facade when another chunk imports its implementation.
    // Wrap-all mode computes no prediction and splits unconditionally. The wrapping policy is
    // carried on the analysis (decided once in `analyze_execution_order`) rather than re-read here.
    let on_demand = analysis.on_demand;
    let mut imported_chunks = FxHashSet::default();
    for (chunk_idx, importee_chunks) in analysis.import_edges.iter_enumerated() {
      imported_chunks
        .extend(importee_chunks.iter().copied().filter(|importee| *importee != chunk_idx));
    }
    // A dynamic import evaluates its target's chunk, so an inline entry trigger hosted there
    // would run the entry's whole program during that load (e.g. a manual group placing a
    // dynamic target next to an entry). Predicted static edges cannot see these loads; collect
    // the cross-chunk dynamic-import targets directly.
    let mut dynamic_target_modules_by_chunk: FxHashMap<ChunkIdx, FxHashSet<ModuleIdx>> =
      FxHashMap::default();
    for module in
      self.link_output.module_table.modules.iter().filter_map(|module| module.as_normal())
    {
      if !self.link_output.metas[module.idx].is_included {
        continue;
      }
      let importer_chunk = chunk_graph.module_to_chunk[module.idx];
      for rec in &module.import_records {
        if rec.kind != ImportKind::DynamicImport {
          continue;
        }
        let Some(importee_idx) = rec.resolved_module else { continue };
        if !self.link_output.module_table[importee_idx].is_normal()
          || !self.link_output.metas[importee_idx].is_included
        {
          continue;
        }
        let Some(importee_chunk) = chunk_graph.module_to_chunk[importee_idx] else { continue };
        if importer_chunk == Some(importee_chunk) {
          continue;
        }
        dynamic_target_modules_by_chunk.entry(importee_chunk).or_default().insert(importee_idx);
      }
    }
    entries_to_split.extend(self.link_output.entries.keys().copied().filter(|entry_module_idx| {
      !matches!(self.link_output.metas[*entry_module_idx].wrap_kind(), WrapKind::None)
        && (!on_demand
          || chunk_graph.entry_module_to_entry_chunk.get(entry_module_idx).is_some_and(
            |entry_chunk_idx| {
              imported_chunks.contains(entry_chunk_idx)
                // A dynamic import of the entry module itself must run its program, so only
                // other hosted targets force the split.
                || dynamic_target_modules_by_chunk.get(entry_chunk_idx).is_some_and(|targets| {
                  targets.iter().any(|target| target != entry_module_idx)
                })
            },
          ))
    }));
    entries_to_split.sort_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order());
    entries_to_split.dedup();

    let mut created = false;
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
      created = true;
    }
    created
  }

  fn restore_order_wrap_entry_facades(&self, chunk_graph: &mut ChunkGraph, plan: &OrderWrapPlan) {
    if self.options.code_splitting.is_disabled() {
      return;
    }

    let entries_to_restore = plan
      .modules()
      .filter(|module_idx| self.link_output.entries.contains_key(module_idx))
      .sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order())
      .collect_vec();

    for entry_module_idx in entries_to_restore {
      let facade_chunk_indices = chunk_graph
        .chunk_table
        .iter_enumerated()
        .filter_map(|(chunk_idx, chunk)| match chunk.kind {
          ChunkKind::EntryPoint { meta, module, .. }
            if module == entry_module_idx
              && chunk.modules.is_empty()
              && meta.intersects(ChunkMeta::DynamicImported | ChunkMeta::EmittedChunk)
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
        .collect_vec();
      let Some(&facade_chunk_idx) = facade_chunk_indices.first() else {
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

      for facade_chunk_idx in facade_chunk_indices {
        chunk_graph.post_chunk_optimization_operations.remove(&facade_chunk_idx);
      }
    }
  }

  /// Normalize the runtime module onto a standalone chunk, then re-run the runtime-chunk merge
  /// proof with the post-lowering consumer set.
  ///
  /// The baseline `try_merge_runtime_chunk` calls run before order analysis, so a pre-lowering
  /// merge never proved anything about the helper demand the wrappers and overlays above added.
  /// Evicting a co-hosted runtime first restores the standalone shape that proof requires; by this
  /// point lowering has materialized every order-introduced demand in [`OrderWrapState`], so the
  /// re-proof sees the complete consumer set.
  fn ensure_runtime_module_for_order_wraps(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    order_state: &OrderWrapState,
  ) {
    let runtime_idx = self.link_output.runtime.id();
    if let Some(runtime_chunk_idx) = chunk_graph.module_to_chunk[runtime_idx] {
      if self.options.code_splitting.is_disabled() {
        return;
      }
      let runtime_chunk = &chunk_graph.chunk_table[runtime_chunk_idx];
      if runtime_chunk.modules.len() == 1 {
        self.clear_module_symbol_chunk_indices(runtime_idx);
        // Facade restoration above can tombstone chunks the pre-lowering merge counted as
        // consumers, so a standalone runtime may only now have a sole consumer left.
        self.fold_runtime_chunk_after_order_lowering(chunk_graph, order_state);
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
      self.fold_runtime_chunk_after_order_lowering(chunk_graph, order_state);
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
    self.fold_runtime_chunk_after_order_lowering(chunk_graph, order_state);
  }

  /// Re-run the runtime-chunk merge proof against the post-lowering consumer set: the
  /// order-introduced consumers from [`OrderWrapState`] plus the merge's own re-scan of every
  /// pre-lowering consumer. Restricted to a sole-consumer host — see
  /// [`RuntimeMergeCascade::SingleConsumerOnly`].
  ///
  /// Esm output only. Under cjs output, `compute_cross_chunk_links` later gives every ESM-exports
  /// entry chunk — including the zero-module facades minted above — a `__toCommonJS` demand that
  /// is invisible here, so a fold could hand an entry chunk with no demand at all a brand-new
  /// require edge into a user chunk. Other formats keep the standalone/evicted layout.
  fn fold_runtime_chunk_after_order_lowering(
    &self,
    chunk_graph: &mut ChunkGraph,
    order_state: &OrderWrapState,
  ) {
    if !matches!(self.options.format, OutputFormat::Esm) {
      return;
    }
    let order_consumers = order_state.runtime_helper_consumer_chunks(&chunk_graph.module_to_chunk);
    self.try_merge_runtime_chunk(
      chunk_graph,
      Some(&order_consumers),
      RuntimeMergeCascade::SingleConsumerOnly,
    );
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
      .filter_map(|(chunk_idx, _)| chunk_graph.chunk_is_live(chunk_idx).then_some(chunk_idx))
      .collect_vec()
  }

  fn renumber_live_chunks(&self, chunk_graph: &mut ChunkGraph) {
    let live_chunks = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(chunk_idx, _)| chunk_graph.chunk_is_live(*chunk_idx))
      .sorted_by_key(|(chunk_idx, chunk)| (chunk.exec_order, chunk_idx.raw()))
      .map(|(chunk_idx, _)| chunk_idx)
      .collect_vec();

    for (exec_order, chunk_idx) in live_chunks.iter().copied().enumerate() {
      chunk_graph.chunk_table[chunk_idx].exec_order =
        exec_order.try_into().expect("Too many chunks, u32 overflowed.");
    }

    chunk_graph.rebuild_sorted_chunk_idx_vec(true);
  }

  pub(super) fn esm_runtime_helper(&self) -> RuntimeHelper {
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
  output.state.set_consumed_reexport_facades(reexport_usage.consumed_facades.clone());
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
    if order_wrapper_is_reexport_transparent(
      &input.linking[module_idx],
      input.asts[module_idx].as_ref(),
      input.keep_names,
    ) {
      output.state.set_reexport_init_transparent(module_idx);
    }
  }

  // Real lowering runs once per bundle, so it builds its own reverse index here; the fixpoint
  // projector passes the analysis-owned one instead of rebuilding per round.
  let reverse_static_imports = super::order_analysis::reverse_static_import_index(input.modules);
  populate_order_import_overlays(
    input,
    &reexport_usage,
    output.state,
    code_splitting_disabled,
    &reverse_static_imports,
  );
}

/// Mint the per-record [`OrderImportOverlay`]s for the current plan: a wrapper-referencing overlay
/// for a re-export/execution-dependency import of a planned direct target, and a
/// retained-re-export-path overlay for a re-export that itself reaches the plan through a
/// tree-shaken barrel. Split out of [`lower_order_state`] so the emergent-cycle fixpoint projector
/// can populate an identical set of overlays on its probe state — the overlays and the nested
/// re-export records are what let the final metadata pass's `transitive_esm_init_targets` restrict
/// a barrel's hop walk to its retained path, so projection stays byte-faithful to the real
/// registration instead of over-approximating. Reads and writes only the [`OrderWrapState`]; it
/// never mints symbols, so the projector can drive it with each module's namespace ref as a wrapper
/// placeholder.
pub(super) fn populate_order_import_overlays(
  input: &OrderLoweringInput<'_>,
  reexport_usage: &FrozenReexportUsage,
  state: &mut OrderWrapState,
  code_splitting_disabled: bool,
  reverse_static_imports: &oxc_index::IndexVec<ModuleIdx, Vec<ModuleIdx>>,
) {
  // Backward closure of the plan over the reverse static-import index: one walk answers every
  // record's "does this importee's static-import subtree reach a plan member" instead of a
  // per-record DFS.
  let mut reaches_plan = FxHashSet::default();
  super::order_analysis::grow_static_import_backward_closure(
    reverse_static_imports,
    input.plan.modules(),
    &mut reaches_plan,
  );
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
        let retained_reexport_path =
          retained_order_reexport_path(input, reexport_usage, importer_idx, stmt_info_idx, rec_idx);
        if !execution_dependencies.contains(&importee_idx) && retained_reexport_path.is_none() {
          continue;
        }
        let Some(importee) = input.modules[importee_idx].as_normal() else {
          continue;
        };
        if !direct_target_is_planned {
          if let Some(retained_reexport_path) = retained_reexport_path
            && reaches_plan.contains(&importee_idx)
          {
            state.insert_import_overlay(
              OrderImportKey { importer: importer_idx, statement: stmt_info_idx, record: rec_idx },
              OrderImportOverlay::transitive_reexport(retained_reexport_path),
              importer.namespace_object_ref,
              importee.namespace_object_ref,
            );
          }
          continue;
        }
        let Some(init_target) = state.esm_init_target(importee_idx, &input.linking[importee_idx])
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
          state.insert_import_overlay(
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

pub(super) fn collect_frozen_reexport_usage(input: &OrderLoweringInput<'_>) -> FrozenReexportUsage {
  let mut consumed_facades = FxHashSet::default();
  for (used_ref, chain) in input.export_chains {
    if input.used_symbols.contains(used_ref) {
      consumed_facades.extend(chain.iter().copied());
    }
  }

  let mut root_paths =
    FxHashMap::<(ModuleIdx, ImportRecordIdx), Vec<(ModuleIdx, ImportRecordIdx)>>::default();
  for (imported_as_ref, paths) in input.star_reexport_records_by_imported_symbol {
    // A namespace-keyed path (recorded for a whole consumed namespace by
    // `record_namespace_consumed_star_reexport_paths`, or a member read resolving to a
    // namespace-valued binding) is consumed exactly when that namespace object is materialized:
    // an included namespace retains every non-ambiguous export, so its star chains are
    // execution-relevant; symbol-level usedness would conflate routes (a leaf used through a
    // direct import elsewhere must not retain a barrel path nobody consumes).
    let key_is_namespace = input.modules[imported_as_ref.owner]
      .as_normal()
      .is_some_and(|module| module.namespace_object_ref == *imported_as_ref);
    for path in paths {
      let Some(root) = path.first().copied() else {
        continue;
      };
      let consumer_is_used = if key_is_namespace {
        // `namespace_included` here is the provisional pre-wrap value: `finalize_chunk_plan` runs
        // `finalized_module_namespace_ref_usage` before order analysis/lowering and re-runs it
        // only after. The skew is safe — the post-wrap refinement can only ADD namespaces
        // demanded by import overlays (`requires_namespace`: `export *` of a dynamic-exports
        // importee, `require` interop, splitting-disabled dynamic import), and those routes
        // discharge their breadth at runtime through `__reExport`/`__toCommonJS` glue rather
        // than statically routed init forwarding. An opaque `import * as` consumer — the demand
        // this gate exists for — is a link-time fact the provisional pass already observes.
        input.linking[imported_as_ref.owner].namespace_included
      } else {
        input.used_symbols.contains(imported_as_ref)
          || consumed_facades.contains(imported_as_ref)
          || input.linking[root.0]
            .referenced_symbols_by_entry_point_chunk
            .iter()
            .any(|(symbol_ref, _)| symbol_ref == imported_as_ref)
      };
      if consumer_is_used {
        root_paths.entry(root).or_default().extend(path.iter().copied());
        // An ancestor's excluded-hop traversal stops at the first init-owning barrel it meets and
        // delegates the rest of the chain to that barrel's own `init_*`
        // (`collect_order_wrap_esm_init_targets` pushes the owning wrapper without descending).
        // That delegation is only sound if the owning barrel itself carries the remainder as
        // retained evidence, so record each such suffix as that barrel's own root — otherwise its
        // interior hop forwards nothing and the chain's pure leaf is never initialized.
        for (position, record) in path.iter().copied().enumerate().skip(1) {
          if module_owns_reexport_init(input, record.0) {
            root_paths.entry(record).or_default().extend(path[position..].iter().copied());
          }
        }
      }
    }
  }

  let mut nested_records = FxHashSet::default();
  for (root, path) in &mut root_paths {
    path.sort_unstable_by_key(|(module_idx, rec_idx)| (module_idx.index(), rec_idx.index()));
    path.dedup();
    // A record is "nested" only when a wrapped ancestor barrel's traversal walks *through* its
    // importer to reach a deeper wrapped target, so the ancestor already owns that init and the
    // interior record must stay silent. That traversal stops at the first non-transparent wrapped
    // barrel it meets, delegating the rest of the chain to that barrel's own `init_*`. A
    // transparent order wrapper remains a waypoint instead: making it own the hop would let an
    // unrelated consumer of the shared barrel initialize retained leaves too early.
    nested_records.extend(
      path
        .iter()
        .copied()
        .filter(|record| record != root)
        .filter(|(module_idx, _)| !module_owns_reexport_init(input, *module_idx)),
    );
  }

  FrozenReexportUsage { root_paths, nested_records, consumed_facades }
}

/// Whether `module_idx` owns re-export initialization: an interop `WrapKind::Esm` wrapper or a
/// non-transparent order wrapper selected by the plan. A transparent order wrapper has no local
/// executable body or unconditional execution dependency, so retained paths cross it and stay
/// owned by the consuming ancestor instead of becoming shared barrel-wide work.
///
/// Concatenated wrapped modules — which would share their group's init rather than own a standalone
/// one — are not supported on this branch (order wrapping never marks a module
/// `ConcatenateWrappedModuleKind::Inner`/`Root`), so no concatenated-kind guard is needed here.
/// Re-add one if concatenated-wrapper support lands.
fn module_owns_reexport_init(input: &OrderLoweringInput<'_>, module_idx: ModuleIdx) -> bool {
  matches!(input.linking[module_idx].wrap_kind(), WrapKind::Esm)
    || (input.plan.contains(&module_idx)
      && !order_wrapper_is_reexport_transparent(
        &input.linking[module_idx],
        input.asts[module_idx].as_ref(),
        input.keep_names,
      ))
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
