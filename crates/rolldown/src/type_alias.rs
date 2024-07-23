use indexmap::IndexSet;
use oxc::index::IndexVec;
use rolldown_common::{Asset, AssetIdx, Chunk, ChunkIdx, EcmaAstIdx, ModuleIdx, PreliminaryAsset};
use rolldown_ecmascript::EcmaAst;

pub type IndexChunks = IndexVec<ChunkIdx, Chunk>;
pub type IndexChunkToAssets = IndexVec<ChunkIdx, IndexSet<AssetIdx>>;
pub type IndexAssets = IndexVec<AssetIdx, Asset>;
pub type IndexPreliminaryAssets = IndexVec<AssetIdx, PreliminaryAsset>;
pub type IndexEcmaAst = IndexVec<EcmaAstIdx, (EcmaAst, ModuleIdx)>;
