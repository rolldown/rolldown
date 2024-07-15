use arcstr::ArcStr;

use crate::ResourceId;

#[derive(Debug)]
pub struct ModuleInfo {
  pub code: Option<ArcStr>,
  pub id: ResourceId,
  pub is_entry: bool,
  pub importers: Vec<ResourceId>,
  pub dynamic_importers: Vec<ResourceId>,
  pub imported_ids: Vec<ResourceId>,
  pub dynamically_imported_ids: Vec<ResourceId>,
}
