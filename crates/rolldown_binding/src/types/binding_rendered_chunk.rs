use std::collections::HashMap;

use napi::{bindgen_prelude::ToNapiValue, Env};
use napi_derive::napi;
use rolldown_common::{ModuleId, RenderedModule};
use rustc_hash::FxHashMap;
use serde::Deserialize;

use super::binding_rendered_module::BindingRenderedModule;

// TODO: should rename BindingRenderedChunk?
#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
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
  // TODO: pass as Map or array then convert on js side?
  #[serde(skip)]
  // #[napi(ts_type = "Record<string, RenderedModule>")]
  // pub modules: HashMap<String, BindingRenderedModule>,
  pub modules: BindingChunkModules,
  pub imports: Vec<String>,
  pub dynamic_imports: Vec<String>,
}

// TODO: also output chunk
// workaround napi's map to_napi_value not handling "\0"
// https://github.com/napi-rs/napi-rs/blob/f116eaf5e54090db4dca8a00ccdb684543a39e86/crates/napi/src/bindgen_runtime/js_values/map.rs#L26
#[napi]
#[derive(Default, Debug)]
pub struct BindingChunkModules(FxHashMap<ModuleId, RenderedModule>);

#[napi]
impl BindingChunkModules {
  #[napi(ts_return_type = "[string, BindingRenderedModule][]")]
  pub fn to_entries(&self, env: Env) -> napi::Result<napi::JsObject> {
    let mut result = env.create_array(0)?;
    for (k, v) in &self.0 {
      let mut entry = env.create_array(2)?;
      entry.set(0, k.to_string())?;
      entry.set(1, Into::<BindingRenderedModule>::into(v.clone()))?;
      result.insert(entry)?;
    }
    result.coerce_to_object()
  }
}

impl napi::bindgen_prelude::FromNapiValue for BindingChunkModules {
  unsafe fn from_napi_value(
    _env: napi::sys::napi_env,
    _napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    Ok(BindingChunkModules::default())
  }
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
      // modules: value
      //   .modules
      //   .into_iter()
      //   .map(|(key, value)| (key.to_string(), value.into()))
      //   .collect(),
      modules: BindingChunkModules(value.modules),
      imports: value.imports.iter().map(|x| x.to_string()).collect(),
      dynamic_imports: value.dynamic_imports.iter().map(|x| x.to_string()).collect(),
    }
  }
}
