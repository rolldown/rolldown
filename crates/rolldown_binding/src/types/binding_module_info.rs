use std::sync::Arc;

use napi_derive::napi;
use rolldown_common::ExportsKind;

#[napi]
pub struct BindingModuleInfo {
  inner: Arc<rolldown_common::ModuleInfo>,
  pub id: String,
  pub importers: Vec<String>,
  pub dynamic_importers: Vec<String>,
  pub imported_ids: Vec<String>,
  pub dynamically_imported_ids: Vec<String>,
  pub exports: Vec<String>,
  pub is_entry: bool,
  #[napi(ts_type = "'es' | 'cjs' | 'unknown'")]
  pub input_format: String,
}

#[napi]
impl BindingModuleInfo {
  pub fn new(inner: Arc<rolldown_common::ModuleInfo>) -> Self {
    let input_format = match inner.input_format {
      ExportsKind::Esm => "es",
      ExportsKind::CommonJs => "cjs",
      ExportsKind::None => "unknown",
    };
    Self {
      id: inner.id.to_string(),
      importers: inner.importers.iter().map(ToString::to_string).collect(),
      dynamic_importers: inner.dynamic_importers.iter().map(ToString::to_string).collect(),
      imported_ids: inner.imported_ids.iter().map(ToString::to_string).collect(),
      dynamically_imported_ids: inner
        .dynamically_imported_ids
        .iter()
        .map(ToString::to_string)
        .collect(),
      is_entry: inner.is_entry,
      exports: inner.exports.iter().map(ToString::to_string).collect(),
      input_format: input_format.to_string(),
      inner,
    }
  }

  #[napi(getter)]
  pub fn code(&self) -> Option<&str> {
    self.inner.code.as_deref()
  }
}
