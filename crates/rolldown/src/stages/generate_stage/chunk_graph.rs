use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Chunk, ChunkIdx, ChunkModulesOrderBy, ChunkTable, EcmaViewMeta, ModuleIdx, RuntimeHelper,
  SymbolRef,
};
use rustc_hash::FxHashMap;

use crate::{SharedOptions, stages::link_stage::LinkStageOutput};

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
  pub chunk_idx_to_reference_ids: FxHashMap<ChunkIdx, Vec<ArcStr>>,
}

impl ChunkGraph {
  pub fn new(modules_len: usize) -> Self {
    Self {
      chunk_table: ChunkTable::default(),
      module_to_chunk: index_vec![None; modules_len],
      sorted_chunk_idx_vec: Vec::new(),
      entry_module_to_entry_chunk: FxHashMap::default(),
      finalized_cjs_ns_map_idx_vec: index_vec![],
      chunk_idx_to_reference_ids: FxHashMap::default(),
    }
  }

  #[expect(unused)]
  pub fn sorted_chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.sorted_chunk_idx_vec.iter().map(move |&id| &self.chunk_table.chunks[id])
  }

  pub fn add_chunk(&mut self, chunk: Chunk) -> ChunkIdx {
    let idx = self.chunk_table.push(chunk);
    self.finalized_cjs_ns_map_idx_vec.push(FxHashMap::default());
    idx
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
        if module.id().starts_with("rolldown:") {
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
      side_effects_free_leaf_modules.sort_by_key(|idx| link_output.module_table[*idx].id());
      rest.sort_unstable_by_key(|idx| link_output.module_table[*idx].exec_order());
      runtime_related.sort_unstable_by_key(|idx| link_output.module_table[*idx].exec_order());

      chunk.modules = runtime_related
        .into_iter()
        .chain(side_effects_free_leaf_modules.into_iter().chain(rest))
        .collect_vec();
    });
  }
}
