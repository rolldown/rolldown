use oxc_index::IndexVec;
use rolldown_common::{Asset, ChunkIdx, InsChunkIdx, InstantiatedChunk, ModuleIdx};
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::indexmap::FxIndexSet;

pub type IndexChunkToInstances = IndexVec<ChunkIdx, FxIndexSet<InsChunkIdx>>;
pub type AssetVec = Vec<Asset>;
pub type IndexInstantiatedChunks = IndexVec<InsChunkIdx, InstantiatedChunk>;
pub type IndexEcmaAst = IndexVec<ModuleIdx, Option<EcmaAst>>;
