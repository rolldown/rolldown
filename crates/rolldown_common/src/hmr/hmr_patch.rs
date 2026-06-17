use std::sync::Arc;

use super::hmr_boundary_output::HmrBoundaryOutput;
use crate::OutputAsset;

#[derive(Debug)]
pub struct HmrPatch {
  pub code: String,
  pub filename: String,
  pub sourcemap: Option<String>,
  pub sourcemap_filename: Option<String>,
  pub hmr_boundaries: Vec<HmrBoundaryOutput>,
  /// Assets emitted while computing this patch (e.g. an image newly imported by
  /// an HMR edit). An HMR update runs no `generate`, so these would otherwise
  /// never reach the served output; the consumer must register them so the URLs
  /// the patch references resolve on the first request. See
  /// `meta/design/plugin-asset-module.md` (rolldown#9812 / vitejs/vite#22596).
  pub assets: Vec<Arc<OutputAsset>>,
}
