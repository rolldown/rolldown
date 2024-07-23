use arcstr::ArcStr;

use crate::ModuleId;

#[derive(Debug)]
pub struct ModuleInfo {
  pub code: Option<ArcStr>,
  pub id: ModuleId,
  pub is_entry: bool,
  pub importers: Vec<ModuleId>,
  pub dynamic_importers: Vec<ModuleId>,
  pub imported_ids: Vec<ModuleId>,
  pub dynamically_imported_ids: Vec<ModuleId>,
}
