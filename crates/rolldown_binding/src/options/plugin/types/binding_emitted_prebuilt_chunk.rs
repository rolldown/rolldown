use arcstr::ArcStr;

use crate::types::binding_sourcemap::BindingSourcemap;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingEmittedPrebuiltChunk {
  pub file_name: String,
  pub name: Option<String>,
  pub code: String,
  pub exports: Option<Vec<String>>,
  pub map: Option<BindingSourcemap>,
  pub sourcemap_file_name: Option<String>,
  pub facade_module_id: Option<String>,
  pub is_entry: Option<bool>,
  pub is_dynamic_entry: Option<bool>,
}

impl TryFrom<BindingEmittedPrebuiltChunk> for rolldown_common::EmittedPrebuiltChunk {
  type Error = anyhow::Error;

  fn try_from(value: BindingEmittedPrebuiltChunk) -> Result<Self, Self::Error> {
    Ok(Self {
      file_name: ArcStr::from(value.file_name),
      name: value.name.map(ArcStr::from),
      code: value.code,
      exports: value.exports.unwrap_or_default(),
      map: value.map.map(TryInto::try_into).transpose()?,
      sourcemap_filename: value.sourcemap_file_name,
      facade_module_id: value.facade_module_id.map(ArcStr::from),
      is_entry: value.is_entry.unwrap_or(false),
      is_dynamic_entry: value.is_dynamic_entry.unwrap_or(false),
    })
  }
}
