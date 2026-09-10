use sugar_path::SugarPath as _;

use crate::ViteImportGlobPlugin;

impl ViteImportGlobPlugin {
  pub(crate) fn set_globs(&self, id: &str, matchers: Vec<GlobMatcher>) {
    if matchers.is_empty() {
      self.glob_matchers.remove(id);
    } else {
      self.glob_matchers.insert(arcstr::ArcStr::from(id), matchers);
    }
  }

  pub(crate) fn remove_globs(&self, id: &str) {
    if !self.glob_matchers.is_empty() {
      self.glob_matchers.remove(&*id.to_slash_lossy());
    }
  }
}

/// Replays the accept/reject decision of the build-time walk in
/// [`crate::utils::GlobImportVisit`] without re-walking the filesystem, so the two cannot drift.
#[derive(Debug)]
pub struct GlobMatcher {
  /// In original case: the walk itself is never case-folded, only the glob comparison is.
  pub walk_root: String,
  /// `(static prefix, pattern)` pairs as `PathWithGlob` splits them.
  pub positive: Vec<(String, String)>,
  pub negated: Vec<(String, String)>,
  pub exhaustive: bool,
  pub case_sensitive: bool,
}
