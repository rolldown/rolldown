use rolldown_sourcemap::SourceMap;
use rustc_hash::FxHashMap;

use super::rendered_module::RenderedModule;

#[allow(clippy::zero_sized_map_values)]
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
  pub map: Option<SourceMap>,
  pub sourcemap_file_name: Option<String>,
}
