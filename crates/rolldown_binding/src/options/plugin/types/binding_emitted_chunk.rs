use crate::types::preserve_entry_signatures::BindingPreserveEntrySignatures;

#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingEmittedChunk {
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub id: String,
  pub importer: Option<String>,
  pub preserve_entry_signatures: Option<BindingPreserveEntrySignatures>,
}

impl TryFrom<BindingEmittedChunk> for rolldown_common::EmittedChunk {
  type Error = napi::Error;
  fn try_from(value: BindingEmittedChunk) -> Result<Self, Self::Error> {
    Ok(Self {
      name: value.name.map(Into::into),
      file_name: value.file_name.map(Into::into),
      id: value.id,
      importer: value.importer,
      preserve_entry_signatures: value
        .preserve_entry_signatures
        .map(std::convert::TryInto::try_into)
        .transpose()?,
    })
  }
}
