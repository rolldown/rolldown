use std::{collections::HashMap, fmt::Debug};

use derivative::Derivative;
use napi_derive::napi;
use serde::Deserialize;

#[napi(object)]
pub struct BindingRenderedModule {
  pub code: Option<String>,
}

impl Debug for BindingRenderedModule {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingRenderedModule").field("code", &"...").finish()
  }
}

impl From<rolldown_common::RenderedModule> for BindingRenderedModule {
  fn from(value: rolldown_common::RenderedModule) -> Self {
    Self { code: value.code }
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
  #[serde(skip_deserializing)]
  pub modules: HashMap<String, BindingRenderedModule>,
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
