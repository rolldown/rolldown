use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::{RenderedModule, ResourceId};

#[derive(Debug, Clone)]
pub struct RenderedChunk {
  // PreRenderedChunk
  pub name: Arc<str>,
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
