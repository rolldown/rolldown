use std::path::PathBuf;

use arcstr::ArcStr;

use crate::PreliminaryFilename;

#[derive(Debug)]
pub struct CssAssetMeta {
  pub filename: ArcStr,
  pub debug_id: u128,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
}
