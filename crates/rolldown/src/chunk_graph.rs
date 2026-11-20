use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::{IndexVec, index_vec};
use oxc_str::CompactStr;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ChunkModulesOrderBy, ChunkTable, EcmaViewMeta, ModuleIdx,
  PostChunkOptimizationOperation, RuntimeHelper, SymbolRef, WrapKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions, stages::link_stage::LinkStageOutput, types::linking_metadata::LinkingMetadata,
};

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunk_table: ChunkTable,
  pub sorted_chunk_idx_vec: Vec<ChunkIdx>,
  /// Module to chunk that contains the module
  pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
  pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
  /// If the namespace is not merged, `Key` == `Value`.
  /// If the namespace is merged, `Key` is the original namespace symbol, and `Value` is the linked namespace symbol.
  pub finalized_cjs_ns_map_idx_vec: IndexVec<ChunkIdx, FxHashMap<SymbolRef, SymbolRef>>,
  /// The `import_x` and `x_deferred` symbols of `import defer * as ns from './x.cjs'` that the
  /// finalizer does not declare. `merge_deferred_cjs_namespaces` puts each symbol of a group here
  /// except the first one by execution order. The first record declares the shared binding under
  /// the name of the canonical symbol, also when a later record owns that symbol.
  pub merged_deferred_cjs_namespace_refs: FxHashSet<SymbolRef>,
  pub chunk_idx_to_reference_ids: FxHashMap<ChunkIdx, Vec<ArcStr>>,
  pub common_chunk_exported_facade_chunk_namespace: FxHashMap<ChunkIdx, FxHashSet<ModuleIdx>>,
  /// Modules from emitted chunks with AllowExtension that were merged into common chunks.
  /// Their export names should be preserved (not minified).
  pub common_chunk_preserve_export_names_modules: FxHashMap<ChunkIdx, FxHashSet<ModuleIdx>>,
  /// Tracks chunks that have been removed during post-optimization of code splitting.
  ///
  /// Since chunks are stored in an `IndexVec`, we have two options when removing chunks:
  /// 1. Remove them and re-index the entire chunk table
  /// 2. Keep them in place and mark them as dead
  ///
  /// We use the second approach to avoid the overhead of re-indexing at the cost of some extra memory.
  pub post_chunk_optimization_operations: FxHashMap<ChunkIdx, PostChunkOptimizationOperation>,
  /// The deferred modules that a chunk initializes at its imports
  /// (`GenerateStage::plan_lazy_host_inits`).
  pub lazy_host_inits: LazyHostInits,
}

/// An eager importer of a deferred module that runs when its chunk loads. The host of the module
/// does not run it at load (`ChunkGraph::deferred_init_host`). The chunk of the importer
/// initializes the module right after one of its imports, instead of a call in the body of the
/// importer: after the import of the host, or after a later import of an earlier module of the
/// importer.
/// See internal-docs/import-defer/implementation.md ("Init after the import of a host").
#[derive(Debug, Default)]
pub struct LazyHostInits {
  /// For each chunk, the modules that it initializes, in the order of its `static_imports`. The
  /// modules after one import are in the order of the imports of the importer.
  pub inits_by_chunk: FxHashMap<ChunkIdx, Vec<LazyHostInit>>,
  /// `(importer, module)`: the importer does not call the init of the module in its body. An
  /// importer with an order wrapper keeps its call.
  pub moved_calls: FxHashSet<(ModuleIdx, ModuleIdx)>,
  /// `esm` output: the host and the module of each init chunk. An init chunk has no modules. It
  /// imports `init_x` from the host and calls it (`import { init_x } from "./x.js"; init_x();`),
  /// and each chunk of `inits_by_chunk` imports it right after the import of `LazyHostInit::after`.
  pub host_by_init_chunk: FxHashMap<ChunkIdx, (ChunkIdx, ModuleIdx)>,
}

/// A deferred module that a chunk initializes right after one of its imports.
#[derive(Debug, Clone, Copy)]
pub struct LazyHostInit {
  /// The chunk after whose import the init runs: the host, or a later import of an earlier module
  /// of the importer.
  pub after: ChunkIdx,
  /// The host of the module.
  pub host: ChunkIdx,
  pub module: ModuleIdx,
}

impl LazyHostInits {
  /// The modules that the chunk initializes right after its import of `after_idx`, in order.
  pub fn inits_after(
    &self,
    chunk_idx: ChunkIdx,
    after_idx: ChunkIdx,
  ) -> impl Iterator<Item = LazyHostInit> + '_ {
    self
      .inits_by_chunk
      .get(&chunk_idx)
      .into_iter()
      .flatten()
      .filter(move |init| init.after == after_idx)
      .copied()
  }

  /// The name under which the host exports the init function of the module (`init_x`, or a short
  /// name with `minifyInternalExports`).
  pub fn init_export_name(
    chunk_graph: &ChunkGraph,
    link_output: &LinkStageOutput,
    host_idx: ChunkIdx,
    module_idx: ModuleIdx,
  ) -> Option<CompactStr> {
    let host = &chunk_graph.chunk_table[host_idx];
    let symbol_db = &link_output.symbol_db;
    let wrapper_ref = symbol_db.canonical_ref_for(link_output.metas[module_idx].wrapper_ref?);
    host
      .exports_to_other_chunks
      .iter()
      .filter(|(export_ref, _)| symbol_db.canonical_ref_for(**export_ref) == wrapper_ref)
      .flat_map(|(_, names)| names)
      .min_by(|a, b| a.as_str().cmp(b.as_str()))
      .cloned()
  }
}

impl ChunkGraph {
  pub fn new(modules_len: usize) -> Self {
    Self {
      chunk_table: ChunkTable::default(),
      module_to_chunk: index_vec![None; modules_len],
      sorted_chunk_idx_vec: Vec::new(),
      entry_module_to_entry_chunk: FxHashMap::default(),
      finalized_cjs_ns_map_idx_vec: index_vec![],
      merged_deferred_cjs_namespace_refs: FxHashSet::default(),
      chunk_idx_to_reference_ids: FxHashMap::default(),
      common_chunk_exported_facade_chunk_namespace: FxHashMap::default(),
      common_chunk_preserve_export_names_modules: FxHashMap::default(),
      post_chunk_optimization_operations: FxHashMap::default(),
      lazy_host_inits: LazyHostInits::default(),
    }
  }

  pub fn add_chunk(&mut self, chunk: Chunk) -> ChunkIdx {
    let idx = self.chunk_table.push(chunk);
    self.finalized_cjs_ns_map_idx_vec.push(FxHashMap::default());
    idx
  }

  /// Not removed by post-chunk-optimization (order-wrap lowering can remove chunks).
  pub fn chunk_is_live(&self, chunk_idx: ChunkIdx) -> bool {
    self.post_chunk_optimization_operations.get(&chunk_idx)
      != Some(&PostChunkOptimizationOperation::Removed)
  }

  /// For an ESM module of a deferred closure (`import defer` or `import.defer()`), the dynamic
  /// entry chunk that holds the code of the module. The tail of this chunk does not call
  /// `init_x()`: another chunk can import this chunk statically for a deferred access, and the
  /// load must not evaluate the module. Each `import()` of the module then calls `init_x()`
  /// itself. `None` when the module is in a chunk with no such tail (a common chunk behind a
  /// facade) or in a user-defined or emitted entry chunk, whose tail must stay.
  pub fn lazy_dynamic_defer_host(
    &self,
    module_idx: ModuleIdx,
    meta: &LinkingMetadata,
  ) -> Option<ChunkIdx> {
    if !meta.wrapped_by_import_defer || !matches!(meta.wrap_kind(), WrapKind::Esm) {
      return None;
    }
    let chunk_idx = self.module_to_chunk[module_idx]?;
    match self.chunk_table[chunk_idx].kind {
      ChunkKind::EntryPoint { module, meta: chunk_meta, .. }
        if module == module_idx
          && !chunk_meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk) =>
      {
        Some(chunk_idx)
      }
      _ => None,
    }
  }

  /// For an ESM module of a deferred closure, the chunk that holds the code of the module and does
  /// not run the module when it loads: the lazy host of the module (`lazy_dynamic_defer_host`), or
  /// a common chunk. A common chunk has no tail, so only an init function runs the module. An eager
  /// importer in another chunk calls `init_x()` in its body
  /// (`GenerateStage::plan_lazy_host_inits`).
  pub fn deferred_init_host(
    &self,
    module_idx: ModuleIdx,
    meta: &LinkingMetadata,
  ) -> Option<ChunkIdx> {
    if !meta.wrapped_by_import_defer || !matches!(meta.wrap_kind(), WrapKind::Esm) {
      return None;
    }
    let chunk_idx = self.module_to_chunk[module_idx]?;
    if matches!(self.chunk_table[chunk_idx].kind, ChunkKind::Common) {
      return Some(chunk_idx);
    }
    self.lazy_dynamic_defer_host(module_idx, meta)
  }

  /// The module is assigned to a live chunk that still contains it.
  ///
  /// O(1) on purpose: this is called for every canonical referenced symbol of every included
  /// statement under strict execution order (`add_depended_symbol_with_wrapped_esm_init`), so the
  /// former `chunk.modules.contains(&module_idx)` linear scan made the pass quadratic on large
  /// chunks. Dropping the scan is sound because of this invariant:
  ///
  ///   whenever `module_to_chunk[m] == Some(c)` and `c` is live, `c.modules` contains `m`.
  ///
  /// `add_module_to_chunk` establishes it (it sets `module_to_chunk[m]` and pushes `m` onto
  /// `c.modules` together). Every site that later removes a module from a chunk's `modules` list
  /// preserves it by pairing the removal with one of:
  ///   - clearing `module_to_chunk[m]` to `None` (unused-runtime sweep, `sweep_unused_runtime_module`);
  ///   - reassigning `module_to_chunk[m]` to the destination chunk via `add_module_to_chunk`
  ///     (runtime relocation in `ensure_runtime_module_for_order_wraps`, the runtime→target merge,
  ///     and `apply_common_chunk_merges`);
  ///   - marking the emptied source chunk `Removed` (so `chunk_is_live` returns false).
  ///
  /// No site removes a module while leaving `module_to_chunk[m]` pointed at a still-live chunk, so
  /// there is no residual case that would need a separate removed-module set.
  pub fn module_is_in_live_chunk(&self, module_idx: ModuleIdx) -> bool {
    self.module_to_chunk[module_idx].is_some_and(|chunk_idx| self.chunk_is_live(chunk_idx))
  }

  /// Render order: user-defined entries, then emitted entries, each by index and only when
  /// absent from post-chunk operations; other chunks follow execution order.
  pub fn rebuild_sorted_chunk_idx_vec(&mut self, only_live: bool) {
    self.sorted_chunk_idx_vec = self
      .chunk_table
      .iter_enumerated()
      .filter(|(chunk_idx, _)| !only_live || self.chunk_is_live(*chunk_idx))
      .sorted_unstable_by_key(|(index, chunk)| match &chunk.kind {
        ChunkKind::EntryPoint { meta, .. }
          if !self.post_chunk_optimization_operations.contains_key(index)
            && meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk) =>
        {
          (u8::from(!meta.contains(ChunkMeta::UserDefinedEntry)), index.raw())
        }
        _ => (2, chunk.exec_order),
      })
      .map(|(idx, _)| idx)
      .collect();
  }

  pub fn add_module_to_chunk(
    &mut self,
    module_idx: ModuleIdx,
    chunk_idx: ChunkIdx,
    depended_runtime_helper: RuntimeHelper,
  ) {
    self.chunk_table.chunks[chunk_idx].modules.push(module_idx);
    self.module_to_chunk[module_idx] = Some(chunk_idx);
    self.chunk_table.chunks[chunk_idx].depended_runtime_helper.insert(depended_runtime_helper);
  }

  pub fn sort_chunk_modules(&mut self, link_output: &LinkStageOutput, options: &SharedOptions) {
    // Sort modules in each chunk by execution order
    self.chunk_table.iter_mut().for_each(|chunk| {
      if matches!(
        options.experimental.chunk_modules_order.unwrap_or_default(),
        ChunkModulesOrderBy::ExecOrder
      ) {
        chunk.modules.sort_unstable_by_key(|idx| link_output.module_table[*idx].exec_order());
        return;
      }

      // group those leaf module that has no side effects together.
      let mut side_effects_free_leaf_modules = vec![];
      let mut rest = vec![];
      let mut runtime_related = vec![];
      for &module_idx in &chunk.modules {
        let module = &link_output.module_table[module_idx];
        if module_idx == link_output.runtime.id() {
          runtime_related.push(module_idx);
          continue;
        }
        if let Some(normal) = module.as_normal()
          && normal.import_records.is_empty()
          && !normal.meta.contains(EcmaViewMeta::ExecutionOrderSensitive)
        {
          side_effects_free_leaf_modules.push(module_idx);
        } else {
          rest.push(module_idx);
        }
      }
      side_effects_free_leaf_modules
        .sort_unstable_by_key(|idx| link_output.module_table[*idx].id().as_str());
      rest.sort_unstable_by_key(|idx| link_output.module_table[*idx].exec_order());
      runtime_related.sort_unstable_by_key(|idx| link_output.module_table[*idx].exec_order());

      chunk.modules = runtime_related
        .into_iter()
        .chain(side_effects_free_leaf_modules.into_iter().chain(rest))
        .collect_vec();
    });
  }
}
