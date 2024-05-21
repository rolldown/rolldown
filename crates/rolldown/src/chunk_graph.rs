use oxc_index::IndexVec;
use rolldown_common::{Chunk, ChunkId, NormalModuleId};
use rustc_hash::FxHashMap;

use crate::type_alias::IndexChunks;

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunks: IndexChunks,
  pub sorted_chunk_ids: Vec<ChunkId>,
  pub user_defined_entry_chunk_ids: Vec<ChunkId>,
  /// Module to chunk that contains the module
  pub module_to_chunk: IndexVec<NormalModuleId, Option<ChunkId>>,
  pub entry_module_to_entry_chunk: FxHashMap<NormalModuleId, ChunkId>,
}

impl ChunkGraph {
  #[allow(unused)]
  pub fn sorted_chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.sorted_chunk_ids.iter().map(move |&id| &self.chunks[id])
  }
}
