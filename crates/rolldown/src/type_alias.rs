use oxc::index::IndexVec;
use rolldown_common::{Chunk, ChunkId, EcmaModule, EcmaModuleId};

pub type IndexChunks = IndexVec<ChunkId, Chunk>;
pub type IndexNormalModules = IndexVec<EcmaModuleId, EcmaModule>;
