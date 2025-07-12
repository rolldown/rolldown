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
  pub preliminary_filename: PreliminaryFilename,
}

impl InstantiatedChunk {
  pub fn finalize(self, filename: ArcStr) -> Asset {
    Asset {
      originate_from: Some(self.originate_from),
      content: self.content,
      map: self.map,
      meta: self.kind,
      filename,
    }
  }
}
