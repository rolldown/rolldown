use index_vec::IndexVec;
use rolldown_common::ModuleId;

use super::chunk::{ChunkId, ChunksVec};

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunks: ChunksVec,
  pub module_to_chunk: IndexVec<ModuleId, Option<ChunkId>>,
}
