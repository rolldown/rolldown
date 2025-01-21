use arcstr::ArcStr;

use crate::StrOrBytes;

#[derive(Debug, Clone)]
pub struct RollupPreRenderedAsset {
  pub names: Vec<ArcStr>,
  pub original_file_names: Vec<ArcStr>,
  pub source: StrOrBytes,
}
