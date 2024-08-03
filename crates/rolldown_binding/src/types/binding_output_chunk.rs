use std::collections::HashMap;

use napi_derive::napi;
use rolldown_sourcemap::SourceMap;

use crate::types::binding_rendered_module::BindingRenderedModule;

#[napi]
pub struct BindingOutputChunk {
  inner: &'static mut rolldown_common::OutputChunk,
}

#[napi]
impl BindingOutputChunk {
  pub fn new(inner: &'static mut rolldown_common::OutputChunk) -> Self {
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
    self.inner.filename.to_string()
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

  #[napi(setter, js_name = "imports")]
  pub fn set_imports(&mut self, imports: Vec<String>) {
    self.inner.imports = imports.into_iter().map(Into::into).collect();
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

  #[napi(setter, js_name = "code")]
  pub fn set_code(&mut self, code: String) {
    self.inner.code = code;
  }

  #[napi(getter)]
  pub fn map(&self) -> napi::Result<Option<String>> {
    Ok(self.inner.map.as_ref().map(SourceMap::to_json_string))
  }

  #[napi(setter, js_name = "map")]
  pub fn set_map(&mut self, map: String) -> napi::Result<()> {
    self.inner.map = Some(
      SourceMap::from_json_string(map.as_str())
        .map_err(|e| napi::Error::from_reason(format!("{e:?}")))?,
    );
    Ok(())
  }

  #[napi(getter)]
  pub fn sourcemap_file_name(&self) -> Option<String> {
    self.inner.sourcemap_filename.clone()
  }

  #[napi(getter)]
  pub fn preliminary_file_name(&self) -> String {
    self.inner.preliminary_filename.to_string()
  }

  #[napi(getter)]
  pub fn name(&self) -> String {
    self.inner.name.to_string()
  }
}
