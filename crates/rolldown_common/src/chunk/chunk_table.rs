use std::ops::{Deref, DerefMut};

use crate::type_aliases::IndexChunks;

#[derive(Debug, Default)]
pub struct ChunkTable {
  pub chunks: IndexChunks,
}

impl Deref for ChunkTable {
  type Target = IndexChunks;

  fn deref(&self) -> &Self::Target {
    &self.chunks
  }
}

impl DerefMut for ChunkTable {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.chunks
  }
}

impl ChunkTable {
  pub fn new(chunks: IndexChunks) -> Self {
    Self { chunks }
  }
}
