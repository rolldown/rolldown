use std::{collections::HashMap, sync::Arc};

use napi_derive::napi;

use crate::types::binding_rendered_module::BindingRenderedModule;

#[napi]
pub struct BindingOutputChunk {
  inner: Arc<rolldown_common::OutputChunk>,
}

#[napi]
impl BindingOutputChunk {
  pub fn new(inner: Arc<rolldown_common::OutputChunk>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn is_entry(&self) -> bool {
    self.inner.is_entry
  }

  #[napi(getter)]
  pub fn is_dynamic_entry(&self) -> bool {
    self.inner.is_dynamic_entry
  }

  #[napi(getter)]
  pub fn facade_module_id(&self) -> Option<String> {
    self.inner.facade_module_id.as_ref().map(|x| x.to_string())
  }

  #[napi(getter)]
  pub fn module_ids(&self) -> Vec<String> {
    self.inner.module_ids.iter().map(|x| x.to_string()).collect()
  }

  #[napi(getter)]
  pub fn exports(&self) -> Vec<String> {
    self.inner.exports.clone()
  }

  // RenderedChunk
  #[napi(getter)]
  pub fn file_name(&self) -> String {
    self.inner.file_name.to_string()
  }

  #[napi(getter)]
  pub fn modules(&self) -> HashMap<String, BindingRenderedModule> {
    self
      .inner
      .modules
      .clone()
      .into_iter()
      .map(|(key, value)| (key.to_string(), value.into()))
      .collect()
  }

  #[napi(getter)]
  pub fn imports(&self) -> Vec<String> {
    self.inner.imports.iter().map(|x| x.to_string()).collect()
  }

  #[napi(getter)]
  pub fn dynamic_imports(&self) -> Vec<String> {
    self.inner.dynamic_imports.iter().map(|x| x.to_string()).collect()
  }

  // OutputChunk
  #[napi(getter)]
  pub fn code(&self) -> String {
    self.inner.code.clone()
  }

  #[napi(getter)]
  pub fn map(&self) -> napi::Result<Option<String>> {
    self
      .inner
      .map
      .as_ref()
      .map(|v| v.to_json_string().map_err(|e| napi::Error::from_reason(format!("{e:?}"))))
      .transpose()
  }

  #[napi(getter)]
  pub fn sourcemap_file_name(&self) -> Option<String> {
    self.inner.sourcemap_file_name.clone()
  }
}
