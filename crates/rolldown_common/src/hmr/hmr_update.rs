use super::hmr_patch::HmrPatch;

/// The server never decides a *boundary-walk* reload: it ships a superset patch and the
/// client's own graph walk decides per tab whether to hot-apply, skip, or reload itself.
/// `FullReload` remains for invalidations only the server can see — e.g. a tsconfig
/// change re-transforms every governed module, which no patch can represent.
#[derive(Debug, Clone)]
pub enum HmrUpdate {
  Patch(HmrPatch),
  FullReload {
    reason: String,
  },
  /// For the hmr request, there're no actual actions that need to be done.
  Noop,
}

impl HmrUpdate {
  pub fn is_full_reload(&self) -> bool {
    matches!(self, Self::FullReload { .. })
  }
}
