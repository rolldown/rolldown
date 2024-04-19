use std::sync::Arc;

use napi_derive::napi;

#[napi]
pub struct BindingModuleInfo {
  inner: Arc<rolldown_common::ModuleInfo>,
  pub id: String,
}

#[napi]
impl BindingModuleInfo {
  pub fn new(inner: Arc<rolldown_common::ModuleInfo>) -> Self {
    Self { id: inner.id.to_string(), inner }
  }

  #[napi(getter)]
  pub fn code(&self) -> Option<String> {
    self.inner.code.as_ref().map(ToString::to_string)
  }
}
