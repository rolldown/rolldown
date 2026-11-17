use oxc_index::IndexVec;
use rolldown_common::{Asset, ChunkIdx, InsChunkIdx, InstantiatedChunk, ModuleIdx, StmtInfos};
use rolldown_ecmascript::EcmaAst;

pub type IndexChunkToInstance = IndexVec<ChunkIdx, Option<InsChunkIdx>>;
pub type AssetVec = Vec<Asset>;
pub type IndexInstantiatedChunks = IndexVec<InsChunkIdx, InstantiatedChunk>;
pub type IndexEcmaAst = IndexVec<ModuleIdx, Option<EcmaAst>>;
pub type IndexStmtInfos = IndexVec<ModuleIdx, StmtInfos>;
