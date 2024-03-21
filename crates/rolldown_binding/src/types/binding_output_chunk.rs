use std::collections::HashMap;

use derivative::Derivative;
use serde::Deserialize;

use crate::types::binding_rendered_module::BindingRenderedModule;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingOutputChunk {
  // PreRenderedChunk
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: String,
  #[serde(skip_deserializing)]
  pub modules: HashMap<String, BindingRenderedModule>,
  // OutputChunk
  pub code: String,
  pub map: Option<String>,
  pub sourcemap_file_name: Option<String>,
}

impl From<Box<rolldown_common::OutputChunk>> for BindingOutputChunk {
  fn from(chunk: Box<rolldown_common::OutputChunk>) -> Self {
    Self {
      code: chunk.code,
      file_name: chunk.file_name,
      is_entry: chunk.is_entry,
      is_dynamic_entry: chunk.is_dynamic_entry,
      facade_module_id: chunk.facade_module_id,
      modules: chunk.modules.into_iter().map(|(key, value)| (key, value.into())).collect(),
      exports: chunk.exports,
      module_ids: chunk.module_ids,
      map: chunk.map.map(|map| {
        let mut buf = vec![];
        map.to_writer(&mut buf).unwrap();
        unsafe { String::from_utf8_unchecked(buf) }
      }),
      sourcemap_file_name: chunk.sourcemap_file_name,
    }
  }
}
