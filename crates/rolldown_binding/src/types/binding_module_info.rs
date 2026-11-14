use std::sync::Arc;

use napi_derive::napi;
use rolldown_common::{ExportsKind, SharedSavedJsModuleOptions};

use super::binding_saved_module_options::{BindingSavedModuleOptions, stamp_from_js};
use crate::options::plugin::types::binding_shared_string::BindingSharedString;

#[napi]
pub struct BindingModuleInfo {
  inner: Arc<rolldown_common::ModuleInfo>,
  saved_js_module_options: Option<SharedSavedJsModuleOptions>,
  #[napi(ts_type = "string")]
  pub id: BindingSharedString,
  #[napi(ts_type = "Array<string>")]
  pub importers: Vec<BindingSharedString>,
  #[napi(ts_type = "Array<string>")]
  pub dynamic_importers: Vec<BindingSharedString>,
  #[napi(ts_type = "Array<string>")]
  pub imported_ids: Vec<BindingSharedString>,
  #[napi(ts_type = "Array<string>")]
  pub dynamically_imported_ids: Vec<BindingSharedString>,
  pub exports: Vec<String>,
  pub is_entry: bool,
  #[napi(ts_type = "'es' | 'cjs' | 'unknown'")]
  pub input_format: String,
}

#[napi]
impl BindingModuleInfo {
  pub fn new(inner: Arc<rolldown_common::ModuleInfo>) -> Self {
    Self::with_saved_js_module_options(inner, None)
  }

  /// `saved_js_module_options` lets the JavaScript side take the values of this module that
  /// came from a saved result of the persistent cache. See `saved_module_options`.
  pub fn with_saved_js_module_options(
    inner: Arc<rolldown_common::ModuleInfo>,
    saved_js_module_options: Option<&SharedSavedJsModuleOptions>,
  ) -> Self {
    let input_format = match inner.input_format {
      ExportsKind::Esm => "es",
      ExportsKind::CommonJs => "cjs",
      ExportsKind::None => "unknown",
    };
    Self {
      id: BindingSharedString::from(inner.id.as_arc_str().clone()),
      importers: inner
        .importers
        .iter()
        .map(|id| BindingSharedString::from(id.as_arc_str().clone()))
        .collect(),
      dynamic_importers: inner
        .dynamic_importers
        .iter()
        .map(|id| BindingSharedString::from(id.as_arc_str().clone()))
        .collect(),
      imported_ids: inner
        .imported_ids
        .iter()
        .map(|id| BindingSharedString::from(id.as_arc_str().clone()))
        .collect(),
      dynamically_imported_ids: inner
        .dynamically_imported_ids
        .iter()
        .map(|id| BindingSharedString::from(id.as_arc_str().clone()))
        .collect(),
      is_entry: inner.is_entry,
      exports: inner.exports.iter().map(ToString::to_string).collect(),
      input_format: input_format.to_string(),
      inner,
      saved_js_module_options: saved_js_module_options.cloned(),
    }
  }

  /// The values that hooks set for this module on the JavaScript side, from saved results that
  /// the persistent cache reused. Only the values that are newer than the stamp `after`.
  #[napi]
  pub fn saved_module_options(&self, after: f64) -> Option<BindingSavedModuleOptions> {
    let saved = self.saved_js_module_options.as_ref()?;
    saved.newer_than(saved.key_of(self.inner.id.as_str()), stamp_from_js(after)).map(Into::into)
  }

  #[napi(getter)]
  pub fn code(&self) -> Option<&str> {
    self.inner.code.as_deref()
  }
}
