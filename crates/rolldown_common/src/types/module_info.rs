use std::sync::Arc;

use crate::ResourceId;

#[derive(Debug)]
pub struct ModuleInfo {
  pub code: Option<Arc<str>>,
  pub id: ResourceId,
  pub is_entry: bool,
  pub importers: Vec<ResourceId>,
  pub dynamic_importers: Vec<ResourceId>,
  pub imported_ids: Vec<ResourceId>,
  pub dynamically_imported_ids: Vec<ResourceId>,
}
