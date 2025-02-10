use crate::options::BindingEntrySignatures;
use napi::bindgen_prelude::Either;
use rolldown::PreserveEntrySignature;

#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingEmittedChunk {
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub id: String,
  pub importer: Option<String>,
  #[napi(ts_type = "'strict' | 'allow-extension' | 'exports-only' | false")]
  pub preserve_signature: Option<BindingEntrySignatures>,
}

impl From<BindingEmittedChunk> for rolldown_common::EmittedChunk {
  fn from(value: BindingEmittedChunk) -> Self {
    Self {
      name: value.name.map(Into::into),
      file_name: value.file_name.map(Into::into),
      id: value.id,
      importer: value.importer,
      preserve_signature: value
        .preserve_signature
        .map(|item| match item {
          Either::A(item_bool) => PreserveEntrySignature::try_from(item_bool),
          Either::B(item_string) => PreserveEntrySignature::try_from(item_string.as_str()),
        })
        .transpose()
        .unwrap_or(Some(PreserveEntrySignature::ExportsOnly)),
    }
  }
}
