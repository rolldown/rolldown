use derivative::Derivative;
use serde::Deserialize;

use super::{binding_output_asset::BindingOutputAsset, binding_output_chunk::BindingOutputChunk};

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingOutputs {
  pub chunks: Vec<BindingOutputChunk>,
  pub assets: Vec<BindingOutputAsset>,
}

impl From<Vec<rolldown_common::Output>> for BindingOutputs {
  fn from(outputs: Vec<rolldown_common::Output>) -> Self {
    let mut chunks: Vec<BindingOutputChunk> = vec![];
    let mut assets: Vec<BindingOutputAsset> = vec![];

    outputs.into_iter().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => chunks.push(chunk.into()),
      rolldown_common::Output::Asset(asset) => assets.push(asset.into()),
    });

    Self { chunks, assets }
  }
}
