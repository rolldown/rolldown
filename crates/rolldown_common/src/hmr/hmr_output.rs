use arcstr::ArcStr;

#[derive(Default)]
pub struct HmrOutput {
  pub patch: String,
  pub hmr_boundaries: Vec<HmrBoundaryOutput>,
  pub full_reload: bool,
}

#[derive(Debug)]
pub struct HmrBoundaryOutput {
  pub boundary: ArcStr,
  pub accepted_via: ArcStr,
}
