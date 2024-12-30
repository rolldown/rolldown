use std::collections::HashMap;

use arcstr::ArcStr;
use rolldown_utils::rayon::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxBuildHasher;

use crate::types::binding_rendered_module::BindingRenderedModule;

#[napi_derive::napi]
#[derive(Default, Debug)]
pub struct RenderedChunk {
  // PreRenderedChunk
  name: String,
  is_entry: bool,
  is_dynamic_entry: bool,
  facade_module_id: Option<String>,
  module_ids: Vec<String>,
  exports: Vec<String>,
  // RenderedChunk
  file_name: String,
  modules: BindingModules,
  imports: Vec<String>,
  dynamic_imports: Vec<String>,
}

#[napi_derive::napi]
impl RenderedChunk {
  #[napi(getter)]
  pub fn get_name(&self) -> String {
    self.name.clone()
  }

  #[napi(getter)]
  pub fn get_is_entry(&self) -> bool {
    self.is_entry
  }

  #[napi(getter)]
  pub fn get_is_dynamic_entry(&self) -> bool {
    self.is_dynamic_entry
  }

  #[napi(getter)]
  pub fn get_facade_module_id(&self) -> Option<String> {
    self.facade_module_id.clone()
  }

  #[napi(getter)]
  pub fn get_module_ids(&self) -> Vec<String> {
    self.module_ids.clone()
  }

  #[napi(getter)]
  pub fn get_exports(&self) -> Vec<String> {
    self.exports.clone()
  }

  #[napi(getter)]
  pub fn get_file_name(&self) -> String {
    self.file_name.clone()
  }

  #[napi(getter)]
  pub fn get_modules(&self) -> BindingModules {
    self.modules.clone()
  }

  #[napi(getter)]
  pub fn get_imports(&self) -> Vec<String> {
    self.imports.clone()
  }

  #[napi(getter)]
  pub fn get_dynamic_imports(&self) -> Vec<String> {
    self.dynamic_imports.clone()
  }
}

#[napi_derive::napi(object)]
#[derive(Default, Debug, Clone)]
pub struct BindingModules {
  pub value: Vec<BindingRenderedModule>,
  pub id_to_index: HashMap<String, u32, FxBuildHasher>,
}

impl From<rolldown_common::RollupRenderedChunk> for RenderedChunk {
  fn from(value: rolldown_common::RollupRenderedChunk) -> Self {
    Self {
      name: value.name.to_string(),
      is_entry: value.is_entry,
      is_dynamic_entry: value.is_dynamic_entry,
      facade_module_id: value.facade_module_id.map(|x| x.to_string()),
      module_ids: value.module_ids.into_iter().map(|x| x.to_string()).collect(),
      exports: value.exports.into_iter().map(|x| x.to_string()).collect(),
      file_name: value.filename.to_string(),
      modules: value.modules.into(),
      imports: value.imports.iter().map(ArcStr::to_string).collect(),
      dynamic_imports: value.dynamic_imports.iter().map(ArcStr::to_string).collect(),
    }
  }
}
#[allow(clippy::cast_possible_truncation)]
impl From<&rolldown_common::Modules> for BindingModules {
  fn from(modules: &rolldown_common::Modules) -> Self {
    let value = modules.value.par_iter().map(|x| x.clone().into()).collect();
    let id_to_index =
      modules.key_to_index.iter().map(|(key, value)| (key.to_string(), *value as u32)).collect();
    Self { value, id_to_index }
  }
}

#[allow(clippy::cast_possible_truncation)]
impl From<rolldown_common::Modules> for BindingModules {
  fn from(modules: rolldown_common::Modules) -> Self {
    let value = modules.value.into_par_iter().map(std::convert::Into::into).collect();
    let id_to_index = modules
      .key_to_index
      .into_iter()
      .map(|(key, value)| (key.to_string(), value as u32))
      .collect();
    Self { value, id_to_index }
  }
}
