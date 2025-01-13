#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingEmittedChunk {
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub id: String,
  pub importer: Option<String>,
}

impl From<BindingEmittedChunk> for rolldown_common::EmittedChunk {
  fn from(value: BindingEmittedChunk) -> Self {
    Self {
      name: value.name.map(Into::into),
      file_name: value.file_name.map(Into::into),
      id: value.id,
      importer: value.importer,
    }
  }
}
