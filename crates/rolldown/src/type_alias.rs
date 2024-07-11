use oxc::index::IndexVec;
use rolldown_common::{Chunk, ChunkIdx};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;
