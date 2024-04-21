use index_vec::IndexVec;
use rolldown_common::{ChunkId, NormalModuleId};

use crate::type_alias::IndexChunks;

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunks: IndexChunks,
  pub user_defined_entry_chunk_ids: Vec<ChunkId>,
  pub module_to_chunk: IndexVec<NormalModuleId, Option<ChunkId>>,
}
