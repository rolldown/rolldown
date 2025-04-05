use std::{collections::HashMap, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::RollupRenderedChunk;
use rustc_hash::FxHashMap;

use crate::types::binding_rendered_chunk::BindingRenderedChunk;

#[napi_derive::napi]
#[derive(Debug)]
pub struct BindingRenderedChunkMeta {
  inner: Arc<FxHashMap<ArcStr, Arc<RollupRenderedChunk>>>,
}

#[napi_derive::napi]
impl BindingRenderedChunkMeta {
  pub fn new(inner: Arc<FxHashMap<ArcStr, Arc<RollupRenderedChunk>>>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn chunks(&self) -> HashMap<String, BindingRenderedChunk> {
    self
      .inner
      .iter()
      .map(|(filename, chunk)| (filename.to_string(), BindingRenderedChunk::new(Arc::clone(chunk))))
      .collect()
  }
}
