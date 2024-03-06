#[allow(clippy::module_inception)]
pub mod chunk;
mod de_conflict;
pub mod render_chunk;
mod render_chunk_exports;
mod render_chunk_imports;
use index_vec::IndexVec;
use rolldown_common::ChunkId;

use self::chunk::Chunk;

pub type ChunksVec = IndexVec<ChunkId, Chunk>;
