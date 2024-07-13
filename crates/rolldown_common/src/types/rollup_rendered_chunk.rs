use arcstr::ArcStr;
use rustc_hash::FxHashMap;

use crate::{RenderedModule, ResourceId};

// The prefix `Rollup` shows that this is struct is designed for compatibility with Rollup. Adding the `Rollup` prefix to show how much types are only used for compatibility with Rollup.
#[derive(Debug, Clone)]
pub struct RollupRenderedChunk {
  // PreRenderedChunk
  pub name: ArcStr,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<ResourceId>,
  pub module_ids: Vec<ResourceId>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub filename: ResourceId,
  pub modules: FxHashMap<ResourceId, RenderedModule>,
  pub imports: Vec<ResourceId>,
  pub dynamic_imports: Vec<ResourceId>,
}
