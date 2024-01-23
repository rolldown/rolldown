use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub struct RenderedModule {
  // The code of the module is omit at now.
  pub original_length: u32,
  pub rendered_length: u32,
}

#[derive(Debug, Clone)]
pub struct OutputChunk {
  // PreRenderedChunk
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: String,
  pub modules: FxHashMap<String, RenderedModule>,
  // OutputChunk
  pub code: String,
}

#[derive(Debug, Clone)]
pub struct OutputAsset {
  pub file_name: String,
  pub source: String,
}

#[derive(Debug, Clone)]
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
