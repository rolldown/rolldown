use index_vec::IndexVec;
use rolldown_common::{Chunk, ChunkId};

pub type IndexChunks = IndexVec<ChunkId, Chunk>;
