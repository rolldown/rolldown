use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct RenderedModule {
  // The code of the module is omit at now.
  pub original_length: u32,
  pub rendered_length: u32,
}

#[derive(Debug)]
pub struct OutputChunk {
  pub file_name: String,
  pub code: String,
  pub is_entry: bool,
  pub facade_module_id: Option<String>,
  pub modules: FxHashMap<String, RenderedModule>,
}

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
