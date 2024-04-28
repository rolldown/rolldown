use std::sync::Arc;

use napi_derive::napi;

#[napi]
pub struct BindingModuleInfo {
  inner: Arc<rolldown_common::ModuleInfo>,
  pub id: String,
  pub importers: Vec<String>,
  pub dynamic_importers: Vec<String>,
  pub imported_ids: Vec<String>,
  pub dynamically_imported_ids: Vec<String>,
  pub is_entry: bool,
}

#[napi]
impl BindingModuleInfo {
  pub fn new(inner: Arc<rolldown_common::ModuleInfo>) -> Self {
    Self {
      id: inner.id.to_string(),
      importers: inner.importers.iter().map(|id| id.to_string()).collect(),
      dynamic_importers: inner.dynamic_importers.iter().map(|id| id.to_string()).collect(),
      imported_ids: inner.imported_ids.iter().map(|id| id.to_string()).collect(),
      dynamically_imported_ids: inner
        .dynamically_imported_ids
        .iter()
        .map(|id| id.to_string())
        .collect(),
      is_entry: inner.is_entry,
      inner,
    }
  }

  #[napi(getter)]
  pub fn code(&self) -> Option<String> {
    self.inner.code.as_ref().map(ToString::to_string)
  }
}
