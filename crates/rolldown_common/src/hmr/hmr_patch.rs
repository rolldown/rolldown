use super::hmr_boundary_output::HmrBoundaryOutput;

#[derive(Debug)]
pub struct HmrPatch {
  pub code: String,
  pub filename: String,
  pub sourcemap: Option<String>,
  pub sourcemap_filename: Option<String>,
  pub hmr_boundaries: Vec<HmrBoundaryOutput>,
}
