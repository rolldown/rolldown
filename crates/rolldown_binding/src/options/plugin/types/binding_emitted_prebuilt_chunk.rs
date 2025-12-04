use arcstr::ArcStr;

use crate::types::binding_sourcemap::BindingSourcemap;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingEmittedPrebuiltChunk {
  pub file_name: String,
  pub code: String,
  pub exports: Option<Vec<String>>,
  pub map: Option<BindingSourcemap>,
  pub sourcemap_file_name: Option<String>,
}

impl TryFrom<BindingEmittedPrebuiltChunk> for rolldown_common::EmittedPrebuiltChunk {
  type Error = anyhow::Error;

  fn try_from(value: BindingEmittedPrebuiltChunk) -> Result<Self, Self::Error> {
    Ok(Self {
      file_name: ArcStr::from(value.file_name),
      code: value.code,
      exports: value.exports.unwrap_or_default(),
      map: value.map.map(TryInto::try_into).transpose()?,
      sourcemap_filename: value.sourcemap_file_name,
    })
  }
}
