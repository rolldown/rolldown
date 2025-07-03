use std::path::PathBuf;

use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;

use crate::{ChunkIdx, InstantiationKind, PreliminaryFilename, StrOrBytes};

#[derive(Debug)]
/// Assets is final output of the bundling process. Inputs -> Modules -> Chunks -> Assets
pub struct Asset {
  /// This field indicates the chunk that this asset originates from.
  /// A chunk might produce multiple assets, for example, a chunk contains [index.js, index.css, icon.png],
  /// it will produce 3 assets: index.js, index.css, icon.png.
  /// We think these 3 assets originate from that chunk.
  ///
  /// Assets could also be produced without chunks, for example, derived sourcemap files or user-emitted assets.
  /// In this case, `originate_from` is `None`.
  pub originate_from: Option<ChunkIdx>,
  pub content: StrOrBytes,
  pub map: Option<SourceMap>,
  pub meta: InstantiationKind,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
  pub filename: ArcStr,
}
