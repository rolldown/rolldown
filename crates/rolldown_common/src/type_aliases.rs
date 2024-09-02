use oxc::index::IndexVec;

use crate::{Chunk, ChunkIdx};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;
