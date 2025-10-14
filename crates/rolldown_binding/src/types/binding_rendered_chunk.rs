use std::sync::Arc;

use rolldown_common::RollupRenderedChunk;

use crate::types::binding_rendered_module::BindingRenderedModule;

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
  pub values: Vec<BindingRenderedModule>,
  pub keys: Vec<String>,
}

impl From<&rolldown_common::Modules> for BindingModules {
  fn from(modules: &rolldown_common::Modules) -> Self {
    let values = modules.values.iter().map(|x| BindingRenderedModule::new(Arc::clone(x))).collect();
    let keys = modules.keys.iter().map(|x| x.to_string()).collect();
    Self { values, keys }
  }
}
