use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct OutputChunk {
  pub code: String,
  pub file_name: String,
  pub is_entry: bool,
  pub facade_module_id: Option<String>,
}

impl From<rolldown::OutputChunk> for OutputChunk {
  fn from(chunk: rolldown::OutputChunk) -> Self {
    Self {
      code: chunk.code,
      file_name: chunk.file_name,
      is_entry: chunk.is_entry,
      facade_module_id: chunk.facade_module_id,
    }
  }
}
