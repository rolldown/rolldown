use oxc_index::IndexVec;

use crate::chunk::{Chunk, ChunkIdx};

pub type IndexChunks<'text> = IndexVec<ChunkIdx, Chunk<'text>>;
