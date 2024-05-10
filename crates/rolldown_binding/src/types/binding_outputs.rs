use napi_derive::napi;

use super::{binding_output_asset::BindingOutputAsset, binding_output_chunk::BindingOutputChunk};

#[napi]
pub struct BindingOutputs {
  inner: &'static mut Vec<rolldown_common::Output>,
}

#[napi]
impl BindingOutputs {
  pub fn new(inner: &'static mut Vec<rolldown_common::Output>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    let mut chunks: Vec<BindingOutputChunk> = vec![];

    self.inner.iter_mut().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => {
        chunks.push(BindingOutputChunk::new(unsafe { std::mem::transmute(chunk) }));
      }
      rolldown_common::Output::Asset(_) => {}
    });

    chunks
  }

  #[napi(getter)]
  pub fn assets(&mut self) -> Vec<BindingOutputAsset> {
    let mut assets: Vec<BindingOutputAsset> = vec![];

    self.inner.iter_mut().for_each(|o| match o {
      rolldown_common::Output::Asset(asset) => {
        assets.push(BindingOutputAsset::new(unsafe { std::mem::transmute(asset) }));
      }
      rolldown_common::Output::Chunk(_) => {}
    });
    assets
  }
}

#[napi]
pub struct ReadOnlyBindingOutputs {
  inner: Vec<rolldown_common::Output>,
}

#[napi]
impl ReadOnlyBindingOutputs {
  pub fn new(inner: Vec<rolldown_common::Output>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    let mut chunks: Vec<BindingOutputChunk> = vec![];
    // println!("555{:#?}", self.inner);

    self.inner.iter_mut().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => {
        chunks.push(BindingOutputChunk::new(unsafe { std::mem::transmute(chunk) }));
      }
      rolldown_common::Output::Asset(_) => {}
    });
    // println!("444{:?}", self.inner);

    chunks
  }

  #[napi(getter)]
  pub fn assets(&mut self) -> Vec<BindingOutputAsset> {
    let mut assets: Vec<BindingOutputAsset> = vec![];

    self.inner.iter_mut().for_each(|o| match o {
      rolldown_common::Output::Asset(asset) => {
        assets.push(BindingOutputAsset::new(unsafe { std::mem::transmute(asset) }));
      }
      rolldown_common::Output::Chunk(_) => {}
    });
    assets
  }
}
