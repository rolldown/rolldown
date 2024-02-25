use index_vec::IndexVec;
use rolldown_common::NormalModuleId;

use super::chunk::{ChunkId, ChunksVec};

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunks: ChunksVec,
  pub module_to_chunk: IndexVec<NormalModuleId, Option<ChunkId>>,
}
