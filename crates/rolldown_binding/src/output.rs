use std::collections::HashMap;

use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct RenderedModule {
  pub code: Option<String>,
  pub removed_exports: Vec<String>,
  pub rendered_exports: Vec<String>,
  pub original_length: u32,
  pub rendered_length: u32,
}

impl From<rolldown_common::RenderedModule> for RenderedModule {
  fn from(value: rolldown_common::RenderedModule) -> Self {
    Self {
      code: None,
      original_length: value.original_length,
      rendered_length: value.rendered_length,
      removed_exports: vec![],
      rendered_exports: vec![],
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct OutputChunk {
  // PreRenderedChunk
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: String,
  pub modules: HashMap<String, RenderedModule>,
  // OutputChunk
  pub code: String,
}

impl From<Box<rolldown_common::OutputChunk>> for OutputChunk {
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

impl From<Box<rolldown_common::OutputAsset>> for OutputAsset {
  fn from(chunk: Box<rolldown_common::OutputAsset>) -> Self {
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

impl From<Vec<rolldown_common::Output>> for Outputs {
  fn from(outputs: Vec<rolldown_common::Output>) -> Self {
    let mut chunks: Vec<OutputChunk> = vec![];
    let mut assets: Vec<OutputAsset> = vec![];

    outputs.into_iter().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => chunks.push(chunk.into()),
      rolldown_common::Output::Asset(asset) => assets.push(asset.into()),
    });

    Self { chunks, assets }
  }
}
