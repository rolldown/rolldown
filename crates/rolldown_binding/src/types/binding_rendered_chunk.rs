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
  pub fn get_name(&self) -> String {
    self.inner.name.to_string()
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
  pub fn get_facade_module_id(&self) -> Option<String> {
    self.inner.facade_module_id.as_ref().map(|x| x.to_string())
  }

  #[napi(getter)]
  pub fn get_module_ids(&self) -> Vec<String> {
    self.inner.module_ids.iter().map(|x| x.to_string()).collect()
  }

  #[napi(getter)]
  pub fn get_exports(&self) -> Vec<String> {
    self.inner.exports.iter().map(std::string::ToString::to_string).collect()
  }

  #[napi(getter)]
  pub fn get_file_name(&self) -> String {
    self.inner.filename.to_string()
  }

  #[napi(getter)]
  pub fn get_modules(&self) -> BindingModules {
    (&self.inner.modules).into()
  }

  #[napi(getter)]
  pub fn get_imports(&self) -> Vec<String> {
    self.inner.imports.iter().map(arcstr::ArcStr::to_string).collect()
  }

  #[napi(getter)]
  pub fn get_dynamic_imports(&self) -> Vec<String> {
    self.inner.dynamic_imports.iter().map(arcstr::ArcStr::to_string).collect()
  }
}

#[napi_derive::napi(object)]
#[derive(Default, Debug, Clone)]
pub struct BindingModules {
  pub values: Vec<BindingRenderedModule>,
  pub keys: Vec<String>,
}

#[allow(clippy::cast_possible_truncation)]
impl From<&rolldown_common::Modules> for BindingModules {
  fn from(modules: &rolldown_common::Modules) -> Self {
    let values = modules.values.iter().map(|x| BindingRenderedModule::new(Arc::clone(x))).collect();
    let keys = modules.keys.iter().map(|x| x.to_string()).collect();
    Self { values, keys }
  }
}
