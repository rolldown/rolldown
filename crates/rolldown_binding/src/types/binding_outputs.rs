use napi_derive::napi;

use crate::type_aliases::{UniqueArcMutex, WeakRefMutex};

use super::{binding_output_asset::BindingOutputAsset, binding_output_chunk::BindingOutputChunk};

/// The `BindingOutputs` owner `Vec<Output>` the mutable reference, it avoid `Clone` at call `writeBundle/generateBundle` hook, and make it mutable.
#[napi]
pub struct BindingOutputs {
  inner: WeakRefMutex<Vec<rolldown_common::Output>>,
}

#[napi]
impl BindingOutputs {
  pub fn new(inner: WeakRefMutex<Vec<rolldown_common::Output>>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    let mut chunks: Vec<BindingOutputChunk> = vec![];
    self.inner.with_inner(|inner| {
      let mut inner = inner.lock().expect("PoisonError raised");
      inner.iter_mut().for_each(|o| match o {
        rolldown_common::Output::Chunk(chunk) => {
          chunks.push(BindingOutputChunk::new(unsafe { std::mem::transmute(chunk.as_mut()) }));
        }
        rolldown_common::Output::Asset(_) => {}
      });
    });

    chunks
  }

  #[napi(getter)]
  pub fn assets(&mut self) -> Vec<BindingOutputAsset> {
    let mut assets: Vec<BindingOutputAsset> = vec![];

    self.inner.with_inner(|inner| {
      let mut inner = inner.lock().expect("PoisonError raised");
      inner.iter_mut().for_each(|o| match o {
        rolldown_common::Output::Asset(asset) => {
          assets.push(BindingOutputAsset::new(unsafe { std::mem::transmute(asset.as_mut()) }));
        }
        rolldown_common::Output::Chunk(_) => {}
      });
    });
    assets
  }

  #[napi]
  pub fn delete(&mut self, file_name: String) {
    self.inner.with_inner(|inner| {
      let mut inner = inner.lock().expect("PoisonError raised");
      if let Some(index) = inner.iter().position(|o| o.filename() == file_name) {
        inner.remove(index);
      }
    });
  }
}

/// The `FinalBindingOutputs` is used at `write()` or `generate()`, it is similar to `BindingOutputs`, if using `BindingOutputs` has unexpected behavior.
/// TODO find a way to export it gracefully.
#[napi]
pub struct FinalBindingOutputs {
  inner: UniqueArcMutex<Vec<rolldown_common::Output>>,
}

#[napi]
impl FinalBindingOutputs {
  pub fn new(inner: Vec<rolldown_common::Output>) -> Self {
    Self { inner: UniqueArcMutex::new(inner.into()) }
  }

  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    let mut chunks: Vec<BindingOutputChunk> = vec![];
    self.inner.weak_ref().with_inner(|inner| {
      let mut inner = inner.lock().expect("PoisonError raised");
      inner.iter_mut().for_each(|o| match o {
        rolldown_common::Output::Chunk(chunk) => {
          chunks.push(BindingOutputChunk::new(unsafe { std::mem::transmute(chunk.as_mut()) }));
        }
        rolldown_common::Output::Asset(_) => {}
      });
    });

    chunks
  }

  #[napi(getter)]
  pub fn assets(&mut self) -> Vec<BindingOutputAsset> {
    let mut assets: Vec<BindingOutputAsset> = vec![];

    self.inner.weak_ref().with_inner(|inner| {
      let mut inner = inner.lock().expect("PoisonError raised");
      inner.iter_mut().for_each(|o| match o {
        rolldown_common::Output::Asset(asset) => {
          assets.push(BindingOutputAsset::new(unsafe { std::mem::transmute(asset.as_mut()) }));
        }
        rolldown_common::Output::Chunk(_) => {}
      });
    });
    assets
  }
}
