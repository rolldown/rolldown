pub mod chunk;
mod de_conflict;

use index_vec::IndexVec;

use self::chunk::Chunk;

index_vec::define_index_type! {
    pub struct ChunkId = u32;
}

pub type ChunksVec = IndexVec<ChunkId, Chunk>;
