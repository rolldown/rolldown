use crate::{ChunkMeta, ModuleIdx};

#[derive(Debug, Default)]
pub enum ChunkKind {
  EntryPoint {
    meta: ChunkMeta,
    bit: u32,
    module: ModuleIdx,
  },
  #[default]
  Common,
}
