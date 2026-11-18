use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;

use crate::{ChunkIdx, InstantiationKind, StrOrBytes};

#[derive(Debug)]
/// Assets is final output of the bundling process. Inputs -> Modules -> Chunks -> Assets
pub struct Asset {
  /// This field indicates the chunk that this asset originates from.
  ///
  /// Assets could also be produced without chunks, for example, derived sourcemap files or user-emitted assets.
  /// In this case, `originate_from` is `None`.
  pub originate_from: Option<ChunkIdx>,
  pub content: StrOrBytes,
  pub map: Option<SourceMap>,
  pub meta: InstantiationKind,
  pub filename: ArcStr,
}
