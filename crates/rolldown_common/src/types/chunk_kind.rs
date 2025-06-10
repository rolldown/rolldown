use crate::{ChunkMeta, ModuleIdx};

#[derive(Debug)]
pub enum ChunkKind {
  EntryPoint { meta: ChunkMeta, bit: u32, module: ModuleIdx },
  Common,
}

impl Default for ChunkKind {
  fn default() -> Self {
    Self::Common
  }
}
