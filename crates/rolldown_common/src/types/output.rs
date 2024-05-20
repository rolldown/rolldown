use crate::OutputChunk;

#[derive(Debug)]
pub struct OutputAsset {
  pub filename: String,
  pub source: String,
}

#[derive(Debug)]
pub enum Output {
  Chunk(Box<OutputChunk>),
  Asset(Box<OutputAsset>),
}

impl Output {
  pub fn filename(&self) -> &str {
    match self {
      Self::Chunk(chunk) => &chunk.filename,
      Self::Asset(asset) => &asset.filename,
    }
  }

  pub fn content(&self) -> &str {
    match self {
      Self::Chunk(chunk) => &chunk.code,
      Self::Asset(asset) => &asset.source,
    }
  }
}
