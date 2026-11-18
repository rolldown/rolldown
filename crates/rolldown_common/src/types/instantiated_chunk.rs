use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;

use crate::{Asset, ChunkIdx, EcmaAssetMeta, InstantiationKind, PreliminaryFilename, StrOrBytes};

/// The rendered form of a `Chunk`.
#[derive(Debug)]
pub struct InstantiatedChunk {
  pub originate_from: ChunkIdx,
  pub content: StrOrBytes,
  pub map: Option<SourceMap>,
  pub ecma_meta: Box<EcmaAssetMeta>,
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
      meta: InstantiationKind::Ecma(self.ecma_meta),
      filename,
    }
  }
}
