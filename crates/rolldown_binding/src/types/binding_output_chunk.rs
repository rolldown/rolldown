use std::collections::HashMap;

use napi_derive::napi;
use rolldown_sourcemap::SourceMap;

use super::{binding_rendered_module::BindingRenderedModule, binding_sourcemap::BindingSourcemap};

// Here using `napi` `getter` fields to avoid the cost of serialize larger data to js side.

#[napi]
pub struct BindingOutputChunk {
  inner: rolldown_common::OutputChunk,
}

#[napi]
impl BindingOutputChunk {
  pub fn new(inner: rolldown_common::OutputChunk) -> Self {
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
    Ok(self.inner.map.as_ref().map(SourceMap::to_json_string))
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

#[napi(object)]
pub struct JsOutputChunk {
  // PreRenderedChunk
  pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub filename: String,
  pub modules: HashMap<String, BindingRenderedModule>,
  pub imports: Vec<String>,
  pub dynamic_imports: Vec<String>,
  // OutputChunk
  pub code: String,
  pub map: Option<BindingSourcemap>,
  pub sourcemap_filename: Option<String>,
  pub preliminary_filename: String,
}

impl TryFrom<JsOutputChunk> for rolldown_common::OutputChunk {
  type Error = anyhow::Error;

  fn try_from(chunk: JsOutputChunk) -> Result<Self, Self::Error> {
    Ok(Self {
      name: chunk.name.into(),
      is_entry: chunk.is_entry,
      is_dynamic_entry: chunk.is_dynamic_entry,
      facade_module_id: chunk.facade_module_id.map(Into::into),
      module_ids: chunk.module_ids.into_iter().map(Into::into).collect(),
      exports: chunk.exports,
      filename: chunk.filename.into(),
      modules: chunk.modules.into_iter().map(|(key, value)| (key.into(), value.into())).collect(),
      imports: chunk.imports.into_iter().map(Into::into).collect(),
      dynamic_imports: chunk.dynamic_imports.into_iter().map(Into::into).collect(),
      code: chunk.code,
      map: chunk.map.map(TryInto::try_into).transpose()?,
      sourcemap_filename: chunk.sourcemap_filename,
      preliminary_filename: chunk.preliminary_filename,
    })
  }
}
