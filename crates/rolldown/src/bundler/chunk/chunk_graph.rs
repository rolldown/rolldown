use index_vec::IndexVec;
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashSet;

use super::{ChunkId, ChunksVec};

#[derive(Debug, Default, Clone)]
pub struct ChunkMeta {
  pub imports: FxHashSet<SymbolRef>,
  pub exports: FxHashSet<SymbolRef>,
}

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunks: ChunksVec,
  pub module_to_chunk: IndexVec<ModuleId, Option<ChunkId>>,
}
