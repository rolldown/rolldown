use arcstr::ArcStr;

use crate::ModuleId;

#[derive(Debug, Clone)]
pub struct RollupPreRenderedChunk {
  pub name: ArcStr,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<ModuleId>,
  pub module_ids: Vec<ModuleId>,
  pub exports: Vec<String>,
}
