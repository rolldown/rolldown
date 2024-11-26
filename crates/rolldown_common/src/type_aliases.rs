use oxc_index::IndexVec;

use crate::{Chunk, ChunkIdx};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;
