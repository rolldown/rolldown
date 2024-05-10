use crate::OutputChunk;

#[derive(Debug)]
pub struct OutputAsset {
  pub file_name: String,
  pub source: String,
}

#[derive(Debug)]
pub enum Output {
  Chunk(Box<OutputChunk>),
  Asset(Box<OutputAsset>),
}

impl Output {
  pub fn file_name(&self) -> &str {
    match self {
      Self::Chunk(chunk) => &chunk.file_name,
      Self::Asset(asset) => &asset.file_name,
    }
  }

  pub fn content(&self) -> &str {
    match self {
      Self::Chunk(chunk) => &chunk.code,
      Self::Asset(asset) => &asset.source,
    }
  }
}
