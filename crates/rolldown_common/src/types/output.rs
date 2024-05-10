use crate::OutputChunk;

#[derive(Debug, Clone)]
pub struct OutputAsset {
  pub file_name: String,
  pub source: String,
}

#[derive(Debug, Clone)]
pub enum Output {
  Chunk(OutputChunk),
  Asset(OutputAsset),
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
