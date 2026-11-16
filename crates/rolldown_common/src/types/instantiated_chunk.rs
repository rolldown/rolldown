use arcstr::ArcStr;
use rolldown_sourcemap::PackedSourceMap;

use crate::{Asset, ChunkIdx, InstantiationKind, PreliminaryFilename, StrOrBytes};

/// `InstantiatedChunk`s are derived from `Chunk`s. Different `InstantiatedChunk`s can be derived from the same `Chunk`
/// by different `Generator`s.
#[derive(Debug)]
pub struct InstantiatedChunk {
  pub originate_from: ChunkIdx,
  pub content: StrOrBytes,
  /// Held packed: about 3 bytes for each token. Unpack it to read its tokens.
  pub map: Option<PackedSourceMap>,
  pub kind: InstantiationKind,
  pub augment_chunk_hash: Option<String>,
  pub preliminary_filename: PreliminaryFilename,
  pub preliminary_sourcemap_filename: Option<PreliminaryFilename>,

  pub post_banner: Option<String>,
  pub post_footer: Option<String>,
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
