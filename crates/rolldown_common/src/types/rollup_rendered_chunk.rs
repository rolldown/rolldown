use arcstr::ArcStr;
use rolldown_rstr::Rstr;
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
  pub exports: Vec<Rstr>,
  // RenderedChunk
  pub filename: ArcStr,
  pub modules: FxHashMap<ModuleId, RenderedModule>,
  pub imports: Vec<ArcStr>,
  pub dynamic_imports: Vec<ArcStr>,
  pub debug_id: u128,
}
