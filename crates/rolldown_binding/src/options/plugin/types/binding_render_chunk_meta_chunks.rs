use std::sync::Arc;

use arcstr::ArcStr;
use indexmap::IndexMap;
use rolldown_common::RollupRenderedChunk;
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxBuildHasher;

use crate::types::binding_rendered_chunk::BindingRenderedChunk;

#[napi_derive::napi]
#[derive(Debug)]
pub struct BindingRenderedChunkMeta {
  inner: Arc<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>,
}

#[napi_derive::napi]
impl BindingRenderedChunkMeta {
  pub fn new(inner: Arc<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn chunks(&self) -> IndexMap<String, BindingRenderedChunk, FxBuildHasher> {
    self
      .inner
      .iter()
      .map(|(filename, chunk)| (filename.to_string(), BindingRenderedChunk::new(Arc::clone(chunk))))
      .collect()
  }
}
