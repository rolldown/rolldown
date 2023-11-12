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

impl From<Box<rolldown::OutputChunk>> for OutputChunk {
  fn from(chunk: Box<rolldown::OutputChunk>) -> Self {
    Self {
      code: chunk.code,
      file_name: chunk.file_name,
      is_entry: chunk.is_entry,
      facade_module_id: chunk.facade_module_id,
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct OutputAsset {
  pub file_name: String,
  pub source: String,
}

impl From<Box<rolldown::OutputAsset>> for OutputAsset {
  fn from(chunk: Box<rolldown::OutputAsset>) -> Self {
    Self { source: chunk.source, file_name: chunk.file_name }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct Outputs {
  pub chunks: Vec<OutputChunk>,
  pub assets: Vec<OutputAsset>,
}

impl From<Vec<rolldown::Output>> for Outputs {
  fn from(outputs: Vec<rolldown::Output>) -> Self {
    let mut chunks: Vec<OutputChunk> = vec![];
    let mut assets: Vec<OutputAsset> = vec![];

    outputs.into_iter().for_each(|o| match o {
      rolldown::Output::Chunk(chunk) => chunks.push(chunk.into()),
      rolldown::Output::Asset(asset) => assets.push(asset.into()),
    });

    Self { chunks, assets }
  }
}
