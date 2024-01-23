#[allow(clippy::module_inception)]
pub mod chunk;
mod de_conflict;
pub mod render_chunk;
mod render_chunk_exports;
mod render_chunk_imports;
use index_vec::IndexVec;

use self::chunk::Chunk;

index_vec::define_index_type! {
    pub struct ChunkId = u32;
}

pub type ChunksVec = IndexVec<ChunkId, Chunk>;
