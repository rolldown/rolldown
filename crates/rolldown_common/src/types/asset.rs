use std::path::PathBuf;

use rolldown_sourcemap::SourceMap;

use crate::{RenderedChunk, ResourceId};

pub struct PreliminaryAsset {
  pub code: String,
  pub map: Option<SourceMap>,
  pub rendered_chunk: RenderedChunk,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: ResourceId,
}

impl PreliminaryAsset {
  // TODO(hyf0): will be used later
  #[allow(unused)]
  fn finalize(self) -> Asset {
    Asset {
      code: self.code,
      map: self.map,
      rendered_chunk: self.rendered_chunk,
      augment_chunk_hash: self.augment_chunk_hash,
      file_dir: self.file_dir,
      preliminary_filename: self.preliminary_filename,
    }
  }
}

/// Assets is final output of the bundling process. Inputs -> Modules -> Chunks -> Assets
pub struct Asset {
  pub code: String,
  pub map: Option<SourceMap>,
  pub rendered_chunk: RenderedChunk,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: ResourceId,
}
