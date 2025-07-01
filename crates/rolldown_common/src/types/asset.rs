use std::path::PathBuf;

use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;

use crate::{ChunkIdx, InstantiationKind, PreliminaryFilename, StrOrBytes};

#[derive(Debug)]
/// Assets is final output of the bundling process. Inputs -> Modules -> Chunks -> Assets
pub struct Asset {
  pub originate_from: ChunkIdx,
  pub content: StrOrBytes,
  pub map: Option<SourceMap>,
  pub meta: InstantiationKind,
  pub augment_chunk_hash: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
  pub filename: ArcStr,
}
