use super::{
  binding_output_asset::{BindingOutputAsset, JsOutputAsset},
  binding_output_chunk::{BindingOutputChunk, JsOutputChunk},
};
use napi_derive::napi;

// The `BindingOutputs` take the data to js side, the rust side will not use it anymore.
#[napi]
pub struct BindingOutputs {
  chunks: Vec<BindingOutputChunk>,
  assets: Vec<BindingOutputAsset>,
}

#[napi]
impl BindingOutputs {
  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    std::mem::take(&mut self.chunks)
  }

  #[napi(getter)]
  pub fn assets(&mut self) -> Vec<BindingOutputAsset> {
    std::mem::take(&mut self.assets)
  }
}

impl From<Vec<rolldown_common::Output>> for BindingOutputs {
  fn from(outputs: Vec<rolldown_common::Output>) -> Self {
    let mut chunks = vec![];
    let mut assets = vec![];
    outputs.into_iter().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => {
        chunks.push(BindingOutputChunk::new(*chunk));
      }
      rolldown_common::Output::Asset(asset) => {
        assets.push(BindingOutputAsset::new(*asset));
      }
    });
    Self { chunks, assets }
  }
}

#[napi(object)]

pub struct JsChangedOutputs {
  pub chunks: Vec<JsOutputChunk>,
  pub assets: Vec<JsOutputAsset>,
  pub deleted: Vec<String>,
}

pub fn update_outputs(
  outputs: &mut Vec<rolldown_common::Output>,
  changed: JsChangedOutputs,
) -> anyhow::Result<()> {
  for chunk in changed.chunks {
    if let Some(index) = outputs.iter().position(|o| o.filename() == chunk.filename) {
      outputs[index] = rolldown_common::Output::Chunk(Box::new(chunk.try_into()?));
    }
  }
  for asset in changed.assets {
    if let Some(index) = outputs.iter().position(|o| o.filename() == asset.filename) {
      outputs[index] = rolldown_common::Output::Asset(Box::new(asset.into()));
    }
  }
  for deleted in changed.deleted {
    if let Some(index) = outputs.iter().position(|o| o.filename() == deleted) {
      outputs.remove(index);
    }
  }
  Ok(())
}
