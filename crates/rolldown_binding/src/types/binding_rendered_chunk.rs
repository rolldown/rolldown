use std::collections::HashMap;

use arcstr::ArcStr;
use rolldown_common::{ModuleId, RenderedModule};
use rustc_hash::{FxBuildHasher, FxHashMap};

use super::binding_rendered_module::BindingRenderedModule;

#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct RenderedChunk {
  // PreRenderedChunk
  pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: String,
  pub modules: BindingChunkModules,
  pub imports: Vec<String>,
  pub dynamic_imports: Vec<String>,
}

impl From<rolldown_common::RollupRenderedChunk> for RenderedChunk {
  fn from(value: rolldown_common::RollupRenderedChunk) -> Self {
    Self {
      name: value.name.to_string(),
      is_entry: value.is_entry,
      is_dynamic_entry: value.is_dynamic_entry,
      facade_module_id: value.facade_module_id.map(|x| x.to_string()),
      module_ids: value.module_ids.into_iter().map(|x| x.to_string()).collect(),
      exports: value.exports,
      file_name: value.filename.to_string(),
      modules: value.modules.into(),
      imports: value.imports.iter().map(ArcStr::to_string).collect(),
      dynamic_imports: value.dynamic_imports.iter().map(ArcStr::to_string).collect(),
    }
  }
}

#[napi_derive::napi(transparent)]
#[derive(Default, Debug)]
pub struct BindingChunkModules(HashMap<String, BindingRenderedModule, FxBuildHasher>);

impl From<FxHashMap<ModuleId, RenderedModule>> for BindingChunkModules {
  fn from(value: FxHashMap<ModuleId, RenderedModule>) -> Self {
    Self(value.into_iter().map(|(key, value)| (key.to_string(), value.into())).collect())
  }
}
