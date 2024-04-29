use rustc_hash::FxHashMap;

use crate::{FilePath, RenderedModule};

#[derive(Debug, Clone)]
pub struct RenderedChunk {
  // PreRenderedChunk
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<FilePath>,
  pub module_ids: Vec<FilePath>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: String,
  pub modules: FxHashMap<FilePath, RenderedModule>,
}
