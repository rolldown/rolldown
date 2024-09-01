use indexmap::IndexSet;
use oxc::index::IndexVec;
use rolldown_common::{Asset, AssetIdx, ChunkIdx, EcmaAstIdx, InstantiatedChunk, ModuleIdx};
use rolldown_ecmascript::EcmaAst;

pub type IndexChunkToAssets = IndexVec<ChunkIdx, IndexSet<AssetIdx>>;
pub type IndexAssets = IndexVec<AssetIdx, Asset>;
pub type IndexInstantiatedChunks = IndexVec<AssetIdx, InstantiatedChunk>;
pub type IndexEcmaAst = IndexVec<EcmaAstIdx, (EcmaAst, ModuleIdx)>;
