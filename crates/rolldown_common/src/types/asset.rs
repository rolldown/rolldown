use std::path::PathBuf;

use rolldown_sourcemap::SourceMap;

use crate::{AssetMeta, ResourceId};

pub struct PreliminaryAsset {
  pub content: String,
  pub map: Option<SourceMap>,
  pub meta: AssetMeta,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: ResourceId,
}

impl PreliminaryAsset {
  // TODO(hyf0): will be used later
  #[allow(unused)]
  fn finalize(self) -> Asset {
    Asset {
      content: self.content,
      map: self.map,
      meta: self.meta,
      augment_chunk_hash: self.augment_chunk_hash,
      file_dir: self.file_dir,
      preliminary_filename: self.preliminary_filename,
    }
  }
}

/// Assets is final output of the bundling process. Inputs -> Modules -> Chunks -> Assets
pub struct Asset {
  pub content: String,
  pub map: Option<SourceMap>,
  pub meta: AssetMeta,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: ResourceId,
}
