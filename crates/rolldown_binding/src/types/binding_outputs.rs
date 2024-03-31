use std::sync::Arc;

use napi_derive::napi;

use super::{binding_output_asset::BindingOutputAsset, binding_output_chunk::BindingOutputChunk};

#[napi]
pub struct BindingOutputs {
  inner: Vec<rolldown_common::Output>,
}

#[napi]
impl BindingOutputs {
  pub fn new(inner: Vec<rolldown_common::Output>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn chunks(&self) -> Vec<BindingOutputChunk> {
    let mut chunks: Vec<BindingOutputChunk> = vec![];

    self.inner.iter().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => {
        chunks.push(BindingOutputChunk::new(Arc::clone(chunk)));
      }
      rolldown_common::Output::Asset(_) => {}
    });

    chunks
  }

  #[napi(getter)]
  pub fn assets(&self) -> Vec<BindingOutputAsset> {
    let mut assets: Vec<BindingOutputAsset> = vec![];

    self.inner.iter().for_each(|o| match o {
      rolldown_common::Output::Asset(asset) => {
        assets.push(BindingOutputAsset::new(Arc::clone(asset)));
      }
      rolldown_common::Output::Chunk(_) => {}
    });
    assets
  }
}
