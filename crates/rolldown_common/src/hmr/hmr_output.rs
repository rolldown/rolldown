use arcstr::ArcStr;

#[derive(Default)]
pub struct HmrOutput {
  pub patch: String,
  pub hmr_boundaries: Vec<HmrBoundaryOutput>,
  pub full_reload: bool,
  pub first_invalidated_by: Option<String>,
  pub is_self_accepting: bool,            // only for hmr invalidate
  pub full_reload_reason: Option<String>, // only for hmr invalidate
}

#[derive(Debug)]
pub struct HmrBoundaryOutput {
  pub boundary: ArcStr,
  pub accepted_via: ArcStr,
}
