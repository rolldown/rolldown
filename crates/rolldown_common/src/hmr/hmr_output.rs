use arcstr::ArcStr;

#[derive(Debug)]
pub struct HmrPatch {
  pub code: String,
  pub filename: String,
  pub sourcemap: Option<String>,
  pub sourcemap_filename: Option<String>,
  pub hmr_boundaries: Vec<HmrBoundaryOutput>,
}

#[derive(Debug)]
pub enum HmrUpdate {
  Patch(HmrPatch),
  FullReload {
    reason: String,
  },
  /// For the hmr request, there're no actual actions that need to be done.
  Noop,
}

#[derive(Debug)]
pub struct ClientHmrUpdate {
  pub client_id: String,
  pub update: HmrUpdate,
}

impl HmrUpdate {
  pub fn is_full_reload(&self) -> bool {
    matches!(self, Self::FullReload { .. })
  }
}

#[derive(Debug)]
pub struct HmrBoundaryOutput {
  pub boundary: ArcStr,
  pub accepted_via: ArcStr,
}
