use oxc_index::IndexVec;
use rolldown_common::{Asset, AssetIdx, ChunkIdx, EcmaAstIdx, InstantiatedChunk, ModuleIdx};
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::indexmap::FxIndexSet;

pub type IndexChunkToAssets = IndexVec<ChunkIdx, FxIndexSet<AssetIdx>>;
pub type IndexAssets = IndexVec<AssetIdx, Asset>;
pub type IndexInstantiatedChunks = IndexVec<AssetIdx, InstantiatedChunk>;
pub type IndexEcmaAst = IndexVec<EcmaAstIdx, (EcmaAst, ModuleIdx)>;
