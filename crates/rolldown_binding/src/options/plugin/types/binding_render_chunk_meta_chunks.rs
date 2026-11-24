use std::sync::Arc;

use arcstr::ArcStr;
use indexmap::IndexMap;
use rolldown_common::RollupRenderedChunk;
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxBuildHasher;

use napi::Either;

use crate::types::{
  binding_rendered_chunk::{BindingRenderedChunk, BindingRenderedChunkData, rendered_chunk_data},
  plain_data::PLAIN_DATA,
};

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

/// `BindingRenderedChunkMeta` as plain data.
#[napi_derive::napi(object, object_from_js = false)]
pub struct BindingRenderedChunkMetaData {
  pub chunks: IndexMap<String, BindingRenderedChunkData, FxBuildHasher>,
}

pub fn rendered_chunk_meta_for_js(
  inner: Arc<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>,
) -> Either<BindingRenderedChunkMeta, BindingRenderedChunkMetaData> {
  if !PLAIN_DATA {
    return Either::A(BindingRenderedChunkMeta::new(inner));
  }
  let chunks = inner
    .iter()
    .map(|(filename, chunk)| (filename.to_string(), rendered_chunk_data(chunk)))
    .collect();
  Either::B(BindingRenderedChunkMetaData { chunks })
}
