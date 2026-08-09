use std::sync::Arc;

use napi_derive::napi;
use rolldown_common::ExportsKind;

use crate::options::plugin::types::binding_shared_string::BindingSharedString;

use super::external_memory_status::ExternalMemoryStatus;

#[napi]
pub struct BindingModuleInfo {
  inner: Option<Arc<rolldown_common::ModuleInfo>>,
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
      inner: Some(inner),
    }
  }

  fn try_get_inner(&self) -> napi::Result<&Arc<rolldown_common::ModuleInfo>> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed: this module info's native data was eagerly released after its hook invocation settled. Copy the fields you need during the hook.",
      )
    })
  }

  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self) -> ExternalMemoryStatus {
    // Unlike the `inner`-only boxes, this class also stores per-field
    // `BindingSharedString` clones whose backing strings would otherwise stay
    // pinned by the box until a finalizer runs. Clear them alongside `inner`;
    // the JS side snapshots every field before calling this, so post-drop
    // field reads (which cannot error, being plain fields) only ever see the
    // emptied values.
    self.id = BindingSharedString::from(arcstr::ArcStr::new());
    self.importers = Vec::new();
    self.dynamic_importers = Vec::new();
    self.imported_ids = Vec::new();
    self.dynamically_imported_ids = Vec::new();
    self.exports = Vec::new();
    match self.inner.take() {
      None => ExternalMemoryStatus {
        freed: false,
        reason: Some("Memory has already been freed".to_string()),
      },
      Some(arc) => {
        let strong_count = Arc::strong_count(&arc);
        if strong_count > 1 {
          // Drop our reference, but others exist
          // Arc drops here automatically
          ExternalMemoryStatus {
            freed: false,
            reason: Some(format!(
              "Data has been dropped, but there are {} other strong reference(s) referring to this data on the native side, so the memory may not be released.",
              strong_count - 1
            )),
          }
        } else {
          // Last reference - memory will be freed
          // Arc drops here automatically
          ExternalMemoryStatus { freed: true, reason: None }
        }
      }
    }
  }

  #[napi(getter)]
  pub fn code(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.code.as_deref())
  }
}
