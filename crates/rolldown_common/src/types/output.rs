use std::sync::Arc;

use arcstr::ArcStr;

use crate::{OutputChunk, StrOrBytes};

#[derive(Debug, Clone)]
pub struct OutputAsset {
  pub names: Vec<String>,
  pub original_file_names: Vec<String>,
  pub filename: ArcStr,
  pub source: StrOrBytes,
}

#[derive(Debug, Clone)]
pub enum Output {
  Chunk(Arc<OutputChunk>),
  Asset(Arc<OutputAsset>),
}

impl Output {
  pub fn filename(&self) -> &str {
    match self {
      Self::Chunk(chunk) => &chunk.filename,
      Self::Asset(asset) => &asset.filename,
    }
  }

  pub fn content_as_bytes(&self) -> &[u8] {
    match self {
      Self::Chunk(chunk) => chunk.code.as_bytes(),
      Self::Asset(asset) => asset.source.as_bytes(),
    }
  }
}
