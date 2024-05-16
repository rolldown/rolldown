use rolldown_sourcemap::SourceMap;
use rustc_hash::FxHashMap;

use crate::ResourceId;

use super::rendered_module::RenderedModule;

#[allow(clippy::zero_sized_map_values)]
#[derive(Debug)]
pub struct OutputChunk {
  // PreRenderedChunk
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<ResourceId>,
  pub module_ids: Vec<ResourceId>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: ResourceId,
  pub modules: FxHashMap<ResourceId, RenderedModule>,
  pub imports: Vec<ResourceId>,
  pub dynamic_imports: Vec<ResourceId>,
  // OutputChunk
  pub code: String,
  pub map: Option<SourceMap>,
  pub sourcemap_file_name: Option<String>,
  pub preliminary_file_name: String,
}
