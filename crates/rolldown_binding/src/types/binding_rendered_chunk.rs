use napi::{
  bindgen_prelude::ToNapiValue,
  sys::{napi_env, napi_value},
  Env,
};
use rolldown_common::{ModuleId, RenderedModule};
use rustc_hash::FxHashMap;
use serde::Deserialize;

use super::binding_rendered_module::BindingRenderedModule;

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
  #[serde(skip)]
  #[napi(ts_type = "Record<string, BindingRenderedModule>")]
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
      modules: BindingChunkModules::new(value.modules),
      imports: value.imports.iter().map(|x| x.to_string()).collect(),
      dynamic_imports: value.dynamic_imports.iter().map(|x| x.to_string()).collect(),
    }
  }
}

// use own map wrapper to workaround "\0" issue
// https://github.com/napi-rs/napi-rs/blob/f116eaf5e54090db4dca8a00ccdb684543a39e86/crates/napi/src/bindgen_runtime/js_values/map.rs#L26
#[derive(Default, Debug)]
pub struct BindingChunkModules(FxHashMap<ModuleId, RenderedModule>);

impl BindingChunkModules {
  pub fn new(map: FxHashMap<ModuleId, RenderedModule>) -> Self {
    Self(map)
  }
}

impl napi::bindgen_prelude::FromNapiValue for BindingChunkModules {
  unsafe fn from_napi_value(
    _env: napi::sys::napi_env,
    _napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    unimplemented!()
  }
}

impl ToNapiValue for BindingChunkModules {
  unsafe fn to_napi_value(raw_env: napi_env, val: Self) -> napi::Result<napi_value> {
    let env = Env::from(raw_env);
    let obj = ToNapiValue::to_napi_value(raw_env, env.create_object()?)?;
    for (k, v) in val.0.into_iter() {
      let status = napi::sys::napi_set_property(
        raw_env,
        obj,
        ToNapiValue::to_napi_value(raw_env, k.to_string())?,
        ToNapiValue::to_napi_value(raw_env, Into::<BindingRenderedModule>::into(v.clone()))?,
      );
      if status != napi::sys::Status::napi_ok {
        return Err(napi::Error::from_status(napi::Status::from(status)));
      }
    }
    Ok(obj)
  }
}
