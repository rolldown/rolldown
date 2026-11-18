use crate::{ChunkIdx, ModuleIdx};

/// A static import of a chunk: another chunk, or an external module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticImport {
  Chunk(ChunkIdx),
  External(ModuleIdx),
}
