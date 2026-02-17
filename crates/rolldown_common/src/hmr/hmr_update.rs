use super::hmr_patch::HmrPatch;

#[derive(Debug)]
pub enum HmrUpdate {
  Patch(HmrPatch),
  FullReload {
    reason: String,
    has_skipped_boundary: bool,
  },
  /// For the hmr request, there're no actual actions that need to be done.
  Noop,
}

impl HmrUpdate {
  pub fn is_full_reload(&self) -> bool {
    matches!(self, Self::FullReload { .. })
  }
}
