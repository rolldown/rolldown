use oxc_index::{IndexVec, index_vec};
use rolldown_common::{Chunk, ChunkIdx, ChunkTable, ModuleIdx, ModuleTable, SymbolRef};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunk_table: ChunkTable,
  pub sorted_chunk_idx_vec: Vec<ChunkIdx>,
  /// Module to chunk that contains the module
  pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
  pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
  /// split original map per chunk
  pub safely_merge_cjs_ns_map_idx_vec: IndexVec<ChunkIdx, FxHashMap<ModuleIdx, Vec<SymbolRef>>>,
}

impl ChunkGraph {
  pub fn new(module_table: &ModuleTable) -> Self {
    Self {
      chunk_table: ChunkTable::default(),
      module_to_chunk: index_vec![None; module_table.modules.len()],
      sorted_chunk_idx_vec: Vec::new(),
      entry_module_to_entry_chunk: FxHashMap::default(),
      safely_merge_cjs_ns_map_idx_vec: index_vec![],
    }
  }

  #[allow(unused)]
  pub fn sorted_chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.sorted_chunk_idx_vec.iter().map(move |&id| &self.chunk_table.chunks[id])
  }

  pub fn add_chunk(&mut self, chunk: Chunk) -> ChunkIdx {
    let idx = self.chunk_table.push(chunk);
    self.safely_merge_cjs_ns_map_idx_vec.push(FxHashMap::default());
    idx
  }

  pub fn add_module_to_chunk(&mut self, module_idx: ModuleIdx, chunk_idx: ChunkIdx) {
    self.chunk_table.chunks[chunk_idx].modules.push(module_idx);
    self.module_to_chunk[module_idx] = Some(chunk_idx);
  }
}
