use std::{collections::HashMap, sync::Arc};

use napi_derive::napi;
use rolldown_error::SingleBuildResult;
use rolldown_sourcemap::SourceMap;
use rustc_hash::FxBuildHasher;

use super::{
  binding_rendered_chunk::BindingModules, binding_rendered_module::BindingRenderedModule,
  binding_sourcemap::BindingSourcemap,
};

// Here using `napi` `getter` fields to avoid the cost of serialize larger data to js side.

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
  pub fn facade_module_id(&self) -> Option<&str> {
    self.inner.facade_module_id.as_deref()
  }

  #[napi(getter)]
  pub fn module_ids(&self) -> Vec<&str> {
    self.inner.module_ids.iter().map(AsRef::as_ref).collect()
  }

  #[napi(getter)]
  pub fn exports(&self) -> Vec<&str> {
    self.inner.exports.iter().map(AsRef::as_ref).collect()
  }

  // RenderedChunk
  #[napi(getter)]
  pub fn file_name(&self) -> &str {
    &self.inner.filename
  }

  #[napi(getter)]
  pub fn modules(&self) -> BindingModules {
    (&self.inner.modules).into()
  }

  #[napi(getter)]
  pub fn imports(&self) -> Vec<&str> {
    self.inner.imports.iter().map(AsRef::as_ref).collect()
  }

  #[napi(getter)]
  pub fn dynamic_imports(&self) -> Vec<&str> {
    self.inner.dynamic_imports.iter().map(AsRef::as_ref).collect()
  }

  // OutputChunk
  #[napi(getter)]
  pub fn code(&self) -> &str {
    &self.inner.code
  }

  #[napi(getter)]
  // TODO: claude code - Cannot change to Option<&str>: performs JSON serialization via to_json_string()
  pub fn map(&self) -> napi::Result<Option<String>> {
    Ok(self.inner.map.as_ref().map(SourceMap::to_json_string))
  }

  #[napi(getter)]
  pub fn sourcemap_file_name(&self) -> Option<&str> {
    self.inner.sourcemap_filename.as_deref()
  }

  #[napi(getter)]
  pub fn preliminary_file_name(&self) -> &str {
    &self.inner.preliminary_filename
  }

  #[napi(getter)]
  pub fn name(&self) -> &str {
    &self.inner.name
  }
}

#[napi_derive::napi(object, object_to_js = false)]
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
  pub modules: HashMap<String, BindingRenderedModule, FxBuildHasher>,
  pub imports: Vec<String>,
  pub dynamic_imports: Vec<String>,
  // OutputChunk
  pub code: String,
  pub map: Option<BindingSourcemap>,
  pub sourcemap_filename: Option<String>,
  pub preliminary_filename: String,
}

pub fn update_output_chunk(
  chunk: &mut Arc<rolldown_common::OutputChunk>,
  js_chunk: JsOutputChunk,
) -> SingleBuildResult<()> {
  let old_chunk = (**chunk).clone();
  *chunk = Arc::new(rolldown_common::OutputChunk {
    code: js_chunk.code,
    map: js_chunk.map.map(TryInto::try_into).transpose()?,
    imports: js_chunk.imports.into_iter().map(Into::into).collect(),
    dynamic_imports: js_chunk.dynamic_imports.into_iter().map(Into::into).collect(),
    is_entry: js_chunk.is_entry, // used by nuxt
    filename: js_chunk.filename.into(),
    ..old_chunk
  });
  Ok(())
}
