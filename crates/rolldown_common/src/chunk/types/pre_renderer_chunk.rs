use std::sync::Arc;

use crate::ResourceId;

#[derive(Debug, Clone)]
pub struct PreRenderedChunk {
  pub name: Arc<str>,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<ResourceId>,
  pub module_ids: Vec<ResourceId>,
  pub exports: Vec<String>,
}
