use std::{path::PathBuf, sync::Arc};

use arcstr::ArcStr;

use crate::{PreliminaryFilename, RollupRenderedChunk, StaticImport};

#[derive(Debug)]

pub struct EcmaAssetMeta {
  pub rendered_chunk: Arc<RollupRenderedChunk>,
  pub debug_id: u128,
  /// Static imports of the rendered chunk, including chunk imports and emitted external imports.
  pub static_imports: Vec<StaticImport>,
  // The updated fields of rendered_chunk after the final render
  pub imports: Vec<ArcStr>,
  pub dynamic_imports: Vec<ArcStr>,
  pub sourcemap_filename: Option<String>,
  pub file_dir: PathBuf,
  pub preliminary_filename: PreliminaryFilename,
}
