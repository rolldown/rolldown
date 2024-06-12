use oxc::index::IndexVec;
use rolldown_common::{Chunk, ChunkId, NormalModule, NormalModuleId};

pub type IndexChunks = IndexVec<ChunkId, Chunk>;
pub type IndexNormalModules = IndexVec<NormalModuleId, NormalModule>;
