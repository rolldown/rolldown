use oxc::index::IndexVec;
use rolldown_common::{Chunk, ChunkIdx, ModuleIdx};
use rustc_hash::FxHashMap;

use crate::type_alias::IndexChunks;

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunks: IndexChunks,
  pub sorted_chunk_idx_vec: Vec<ChunkIdx>,
  /// Module to chunk that contains the module
  pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
  pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
}

impl ChunkGraph {
  #[allow(unused)]
  pub fn sorted_chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.sorted_chunk_idx_vec.iter().map(move |&id| &self.chunks[id])
  }
}
