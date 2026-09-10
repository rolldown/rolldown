use std::sync::Arc;

use arcstr::ArcStr;
use indexmap::IndexMap;
use rolldown_common::RollupRenderedChunk;
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxBuildHasher;

use crate::types::binding_rendered_chunk::BindingRenderedChunk;
use crate::types::external_memory_status::ExternalMemoryStatus;

#[napi_derive::napi]
#[derive(Debug)]
pub struct BindingRenderedChunkMeta {
  inner: Option<Arc<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>>,
}

#[napi_derive::napi]
impl BindingRenderedChunkMeta {
  pub fn new(inner: Arc<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&Arc<FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>>> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed: this render-chunk meta's native data was eagerly released after its hook invocation settled. Copy the fields you need during the hook.",
      )
    })
  }

  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self) -> ExternalMemoryStatus {
    match self.inner.take() {
      None => ExternalMemoryStatus {
        freed: false,
        reason: Some("Memory has already been freed".to_string()),
      },
      Some(arc) => {
        let strong_count = Arc::strong_count(&arc);
        if strong_count > 1 {
          ExternalMemoryStatus {
            freed: false,
            reason: Some(format!(
              "Data has been dropped, but there are {} other strong reference(s) referring to this data on the native side, so the memory may not be released.",
              strong_count - 1
            )),
          }
        } else {
          ExternalMemoryStatus { freed: true, reason: None }
        }
      }
    }
  }

  #[napi(getter)]
  pub fn chunks(&self) -> napi::Result<IndexMap<String, BindingRenderedChunk, FxBuildHasher>> {
    Ok(
      self
        .try_get_inner()?
        .iter()
        .map(|(filename, chunk)| {
          (filename.to_string(), BindingRenderedChunk::new(Arc::clone(chunk)))
        })
        .collect(),
    )
  }
}
