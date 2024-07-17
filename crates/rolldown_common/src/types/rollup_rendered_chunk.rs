use arcstr::ArcStr;
use rustc_hash::FxHashMap;

use crate::{ModuleId, RenderedModule};

// The prefix `Rollup` shows that this is struct is designed for compatibility with Rollup. Adding the `Rollup` prefix to show how much types are only used for compatibility with Rollup.
#[derive(Debug, Clone)]
pub struct RollupRenderedChunk {
  // PreRenderedChunk
  pub name: ArcStr,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<ModuleId>,
  pub module_ids: Vec<ModuleId>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub filename: ModuleId,
  pub modules: FxHashMap<ModuleId, RenderedModule>,
  pub imports: Vec<ModuleId>,
  pub dynamic_imports: Vec<ModuleId>,
}
