use std::path::PathBuf;

use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;

use crate::{Asset, ChunkIdx, InstantiationKind, PreliminaryFilename, StrOrBytes};

/// `InstantiatedChunk`s are derived from `Chunk`s. Different `InstantiatedChunk`s can be derived from the same `Chunk`
/// by different `Generator`s.
#[derive(Debug)]
pub struct InstantiatedChunk {
  pub originate_from: ChunkIdx,
  pub content: StrOrBytes,
  pub map: Option<SourceMap>,
  pub kind: InstantiationKind,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
}

impl InstantiatedChunk {
  pub fn finalize(self, filename: ArcStr) -> Asset {
    Asset {
      originate_from: self.originate_from,
      content: self.content,
      map: self.map,
      meta: self.kind,
      augment_chunk_hash: self.augment_chunk_hash,
      file_dir: self.file_dir,
      preliminary_filename: self.preliminary_filename,
      filename,
    }
  }
}
