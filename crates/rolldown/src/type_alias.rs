use indexmap::IndexSet;
use oxc::index::IndexVec;
use rolldown_common::{Asset, AssetIdx, Chunk, ChunkIdx, PreliminaryAsset};

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;
pub type IndexChunkToAssets = IndexVec<ChunkIdx, IndexSet<AssetIdx>>;
pub type IndexAssets = IndexVec<AssetIdx, Asset>;
pub type IndexPreliminaryAssets = IndexVec<AssetIdx, PreliminaryAsset>;
