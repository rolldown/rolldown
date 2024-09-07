use arcstr::ArcStr;

use crate::{AssetSource, OutputChunk};

#[derive(Debug)]
pub struct OutputAsset {
  pub name: Option<String>,
  pub original_file_name: Option<String>,
  pub filename: ArcStr,
  pub source: AssetSource,
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

  pub fn content_as_bytes(&self) -> &[u8] {
    match self {
      Self::Chunk(chunk) => chunk.code.as_bytes(),
      Self::Asset(asset) => asset.source.as_bytes(),
    }
  }
}
