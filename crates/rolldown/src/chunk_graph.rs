use arcstr::ArcStr;
use oxc::index::{index_vec, IndexVec};
use rolldown_common::{Chunk, ChunkIdx, ChunkTable, ModuleIdx, ModuleTable};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunk_table: ChunkTable,
  pub sorted_chunk_idx_vec: Vec<ChunkIdx>,
  /// Module to chunk that contains the module
  pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
  pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
  chunk_idx_by_name: FxHashMap<ArcStr, ChunkIdx>,
}

impl ChunkGraph {
  pub fn new(module_table: &ModuleTable) -> Self {
    Self {
      chunk_table: ChunkTable::default(),
      module_to_chunk: index_vec![None; module_table.modules.len()],
      sorted_chunk_idx_vec: Vec::new(),
      entry_module_to_entry_chunk: FxHashMap::default(),
      chunk_idx_by_name: FxHashMap::default(),
    }
  }

  #[allow(unused)]
  pub fn sorted_chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.sorted_chunk_idx_vec.iter().map(move |&id| &self.chunk_table.chunks[id])
  }

  pub fn add_chunk(&mut self, chunk: Chunk) -> ChunkIdx {
    let idx = self.chunk_table.push(chunk);
    let chunk = &self.chunk_table.chunks[idx];
    if let Some(name) = &chunk.name {
      debug_assert!(
        !self.chunk_idx_by_name.contains_key(name),
        "Should not have duplicate chunk name"
      );
      self.chunk_idx_by_name.insert(name.clone(), idx);
    }
    idx
  }

  pub fn add_module_to_chunk(&mut self, module_idx: ModuleIdx, chunk_idx: ChunkIdx) {
    self.chunk_table.chunks[chunk_idx].modules.push(module_idx);
    self.module_to_chunk[module_idx] = Some(chunk_idx);
  }
}
