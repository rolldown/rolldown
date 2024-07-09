use oxc::index::IndexVec;
use rolldown_common::{Chunk, ChunkIdx, EcmaModule, EcmaModuleIdx};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;
pub type IndexNormalModules = IndexVec<EcmaModuleIdx, EcmaModule>;
