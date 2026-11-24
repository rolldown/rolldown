use std::sync::Arc;

use napi::Either;
use rolldown_common::RollupRenderedChunk;

use crate::types::{
  binding_rendered_module::{
    BindingRenderedModule, BindingRenderedModuleData, rendered_module_for_js,
  },
  plain_data::PLAIN_DATA,
};

#[napi_derive::napi]
#[derive(Debug)]
pub struct BindingRenderedChunk {
  inner: Arc<RollupRenderedChunk>,
}

#[napi_derive::napi]
impl BindingRenderedChunk {
  pub fn new(inner: Arc<RollupRenderedChunk>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn get_name(&self) -> &str {
    &self.inner.name
  }

  #[napi(getter)]
  pub fn get_is_entry(&self) -> bool {
    self.inner.is_entry
  }

  #[napi(getter)]
  pub fn get_is_dynamic_entry(&self) -> bool {
    self.inner.is_dynamic_entry
  }

  #[napi(getter)]
  pub fn get_facade_module_id(&self) -> Option<&str> {
    self.inner.facade_module_id.as_deref()
  }

  #[napi(getter)]
  pub fn get_module_ids(&self) -> Vec<&str> {
    self.inner.module_ids.iter().map(AsRef::as_ref).collect()
  }

  #[napi(getter)]
  pub fn get_exports(&self) -> Vec<&str> {
    self.inner.exports.iter().map(AsRef::as_ref).collect()
  }

  #[napi(getter)]
  pub fn get_file_name(&self) -> &str {
    &self.inner.filename
  }

  #[napi(getter)]
  pub fn get_modules(&self) -> BindingModules {
    (&self.inner.modules).into()
  }

  #[napi(getter)]
  pub fn get_imports(&self) -> Vec<&str> {
    self.inner.imports.iter().map(AsRef::as_ref).collect()
  }

  #[napi(getter)]
  pub fn get_dynamic_imports(&self) -> Vec<&str> {
    self.inner.dynamic_imports.iter().map(AsRef::as_ref).collect()
  }
}

#[napi_derive::napi(object, object_from_js = false)]
#[derive(Default, Debug, Clone)]
pub struct BindingModules {
  pub values: Vec<Either<BindingRenderedModule, BindingRenderedModuleData>>,
  pub keys: Vec<String>,
}

impl From<&rolldown_common::Modules> for BindingModules {
  fn from(modules: &rolldown_common::Modules) -> Self {
    let values = modules.values.iter().map(rendered_module_for_js).collect();
    let keys = modules.keys.iter().map(ToString::to_string).collect();
    Self { values, keys }
  }
}

/// The getters of `BindingRenderedChunk` as plain data.
#[napi_derive::napi(object, object_from_js = false, use_nullable = true)]
#[derive(Debug, Clone)]
pub struct BindingRenderedChunkData {
  pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  pub file_name: String,
  pub modules: BindingModules,
  pub imports: Vec<String>,
  pub dynamic_imports: Vec<String>,
}

pub fn rendered_chunk_for_js(
  inner: Arc<RollupRenderedChunk>,
) -> Either<BindingRenderedChunk, BindingRenderedChunkData> {
  if PLAIN_DATA {
    Either::B(rendered_chunk_data(&inner))
  } else {
    Either::A(BindingRenderedChunk::new(inner))
  }
}

pub fn rendered_chunk_data(inner: &RollupRenderedChunk) -> BindingRenderedChunkData {
  BindingRenderedChunkData {
    name: inner.name.to_string(),
    is_entry: inner.is_entry,
    is_dynamic_entry: inner.is_dynamic_entry,
    facade_module_id: inner.facade_module_id.as_ref().map(ToString::to_string),
    module_ids: inner.module_ids.iter().map(ToString::to_string).collect(),
    exports: inner.exports.iter().map(ToString::to_string).collect(),
    file_name: inner.filename.to_string(),
    modules: (&inner.modules).into(),
    imports: inner.imports.iter().map(ToString::to_string).collect(),
    dynamic_imports: inner.dynamic_imports.iter().map(ToString::to_string).collect(),
  }
}
