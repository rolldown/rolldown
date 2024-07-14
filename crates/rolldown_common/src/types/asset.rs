use std::path::PathBuf;

use rolldown_sourcemap::SourceMap;

use crate::{AssetMeta, ChunkIdx, PreliminaryFilename};

pub struct PreliminaryAsset {
  pub origin_chunk: ChunkIdx,
  pub content: String,
  pub map: Option<SourceMap>,
  pub meta: AssetMeta,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
}

impl PreliminaryAsset {
  pub fn finalize(self, filename: String) -> Asset {
    Asset {
      origin_chunk: self.origin_chunk,
      content: self.content,
      map: self.map,
      meta: self.meta,
      augment_chunk_hash: self.augment_chunk_hash,
      file_dir: self.file_dir,
      preliminary_filename: self.preliminary_filename,
      filename,
    }
  }
}

/// Assets is final output of the bundling process. Inputs -> Modules -> Chunks -> Assets
pub struct Asset {
  pub origin_chunk: ChunkIdx,
  pub content: String,
  pub map: Option<SourceMap>,
  pub meta: AssetMeta,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
  pub filename: String,
}
