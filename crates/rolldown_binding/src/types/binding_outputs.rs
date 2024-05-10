use napi_derive::napi;

use super::{binding_output_asset::BindingOutputAsset, binding_output_chunk::BindingOutputChunk};

/// The `BindingOutputs` owner `Vec<Output>` the mutable reference, it avoid `Clone` at call `writeBundle/generateBundle` hook, and make it mutable.
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
        chunks.push(BindingOutputChunk::new(unsafe { std::mem::transmute(chunk.as_mut()) }));
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
        assets.push(BindingOutputAsset::new(unsafe { std::mem::transmute(asset.as_mut()) }));
      }
      rolldown_common::Output::Chunk(_) => {}
    });
    assets
  }
}

/// The `FinalBindingOutputs` is used at `write()` or `generate()`, it is similar to `BindingOutputs`, if using `BindingOutputs` has unexpected behavior.
/// TODO find a way to export it gracefully.
#[napi]
pub struct FinalBindingOutputs {
  inner: Vec<rolldown_common::Output>,
}

#[napi]
impl FinalBindingOutputs {
  pub fn new(inner: Vec<rolldown_common::Output>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    let mut chunks: Vec<BindingOutputChunk> = vec![];

    self.inner.iter_mut().for_each(|o| match o {
      rolldown_common::Output::Chunk(chunk) => {
        chunks.push(BindingOutputChunk::new(unsafe { std::mem::transmute(chunk.as_mut()) }));
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
        assets.push(BindingOutputAsset::new(unsafe { std::mem::transmute(asset.as_mut()) }));
      }
      rolldown_common::Output::Chunk(_) => {}
    });
    assets
  }
}
