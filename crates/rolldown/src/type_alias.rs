use oxc::index::IndexVec;
use rolldown_common::{Chunk, ChunkIdx, Module, ModuleIdx};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;
pub type IndexNormalModules = IndexVec<ModuleIdx, Module>;
