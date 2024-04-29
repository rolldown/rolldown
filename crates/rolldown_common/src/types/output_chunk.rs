use rolldown_sourcemap::SourceMap;
use rustc_hash::FxHashMap;

use crate::FilePath;

use super::rendered_module::RenderedModule;

#[allow(clippy::zero_sized_map_values)]
#[derive(Debug, Clone)]
pub struct OutputChunk {
  // PreRenderedChunk
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<FilePath>,
  pub module_ids: Vec<FilePath>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: FilePath,
  pub modules: FxHashMap<FilePath, RenderedModule>,
  pub imports: Vec<FilePath>,
  pub dynamic_imports: Vec<FilePath>,
  // OutputChunk
  pub code: String,
  pub map: Option<SourceMap>,
  pub sourcemap_file_name: Option<String>,
}
