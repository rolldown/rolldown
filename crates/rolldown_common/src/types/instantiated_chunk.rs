use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;
use rustc_hash::FxHashMap;

use crate::{Asset, ChunkIdx, EcmaAssetMeta, InstantiationKind, PreliminaryFilename, StrOrBytes};

/// The rendered form of a `Chunk`.
#[derive(Debug)]
pub struct InstantiatedChunk {
  pub originate_from: ChunkIdx,
  pub content: StrOrBytes,
  pub map: Option<SourceMap>,
  pub ecma_meta: Box<EcmaAssetMeta>,
  pub augment_chunk_hash: Option<String>,
  /// See [`ChunkHashDependencies`].
  pub hash_dependencies: Option<String>,
  pub preliminary_filename: PreliminaryFilename,
  pub preliminary_sourcemap_filename: Option<PreliminaryFilename>,

  pub post_banner: Option<String>,
  pub post_footer: Option<String>,
}

/// Used by the chunk import map plugin. Maps the preliminary file name of a chunk to the
/// preliminary file names of the chunks that its hash also depends on.
#[derive(Debug, Default)]
pub struct ChunkHashDependencies(pub FxHashMap<ArcStr, String>);

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
