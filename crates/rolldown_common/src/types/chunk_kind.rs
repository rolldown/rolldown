use crate::{ChunkMeta, ModuleIdx};

#[derive(Debug, Default)]
pub enum ChunkKind {
  EntryPoint {
    meta: ChunkMeta,
    module: ModuleIdx,
  },
  #[default]
  Common,
}
