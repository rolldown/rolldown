use std::sync::Arc;

use crate::{EmittedAsset, OutputChunk};

pub type OutputAsset = EmittedAsset;

impl OutputAsset {
  pub fn filename(&self) -> &str {
    self.filename.as_ref().expect("should have file name")
  }
}

#[derive(Debug)]
pub enum Output {
  Chunk(Box<OutputChunk>),
  Asset(Arc<OutputAsset>),
}

impl Output {
  pub fn filename(&self) -> &str {
    match self {
      Self::Chunk(chunk) => &chunk.filename,
      Self::Asset(asset) => asset.filename.as_ref().expect("should have file name"),
    }
  }

  pub fn content_as_bytes(&self) -> &[u8] {
    match self {
      Self::Chunk(chunk) => chunk.code.as_bytes(),
      Self::Asset(asset) => asset.source.as_bytes(),
    }
  }
}
