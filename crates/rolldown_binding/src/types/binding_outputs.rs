use std::{
  cell::RefCell,
  sync::{Arc, Mutex},
  vec,
};

use napi_derive::napi;

use super::{binding_output_asset::BindingOutputAsset, binding_output_chunk::BindingOutputChunk};

#[napi]
pub struct BindingOutputs {
  inner: Arc<Mutex<RefCell<Vec<rolldown_common::Output>>>>,
}

#[napi]
impl BindingOutputs {
  pub fn new(inner: Arc<Mutex<RefCell<Vec<rolldown_common::Output>>>>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn chunks(&mut self) -> Vec<BindingOutputChunk> {
    let mut chunks: Vec<BindingOutputChunk> = vec![];

    self.inner.lock().expect("should have lock").borrow_mut().iter_mut().for_each(|o| match o {
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

    self.inner.lock().expect("should have lock").borrow_mut().iter_mut().for_each(|o| match o {
      rolldown_common::Output::Asset(asset) => {
        assets.push(BindingOutputAsset::new(unsafe { std::mem::transmute(asset.as_mut()) }));
      }
      rolldown_common::Output::Chunk(_) => {}
    });
    assets
  }

  #[napi]
  pub fn delete(&mut self, file_name: String) {
    let inner = self.inner.lock().expect("should have lock");
    let index = { inner.borrow().iter().position(|o| o.filename() == file_name) };
    if let Some(index) = index {
      inner.borrow_mut().remove(index);
    }
  }

  /// TODO
  /// The napi look like is not drop the strut after the function call, so we need to call it manually.
  #[allow(clippy::should_implement_trait)]
  #[napi]
  pub fn drop(&mut self) {
    let _ = std::mem::replace(&mut self.inner, Arc::new(Mutex::new(RefCell::new(vec![]))));
  }
}
