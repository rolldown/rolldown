use arcstr::ArcStr;
use oxc::span::CompactStr;
use rolldown_utils::indexmap::FxIndexSet;

use crate::ModuleId;

#[derive(Debug)]
pub struct ModuleInfo {
  pub code: Option<ArcStr>,
  pub id: ModuleId,
  pub is_entry: bool,
  pub importers: FxIndexSet<ModuleId>,
  pub dynamic_importers: FxIndexSet<ModuleId>,
  pub imported_ids: FxIndexSet<ModuleId>,
  pub dynamically_imported_ids: FxIndexSet<ModuleId>,
  pub exports: Vec<CompactStr>,
}
