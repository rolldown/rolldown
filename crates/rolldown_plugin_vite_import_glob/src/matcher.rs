use arcstr::ArcStr;

use crate::ViteImportGlobPlugin;

impl ViteImportGlobPlugin {
  /// `id` is a slash-normalized module id.
  pub(crate) fn set_globs(&self, id: &str, matchers: Vec<GlobMatcher>) {
    if matchers.is_empty() {
      self.glob_matchers.remove(id);
    } else {
      self.glob_matchers.insert(ArcStr::from(id), matchers);
    }
  }
}

/// Tells whether a created or deleted path changes the result of one `import.meta.glob` call,
/// without walking the filesystem again.
#[derive(Debug)]
pub struct GlobMatcher {
  /// In original case: the walk itself is never case-folded, only the glob comparison is.
  pub walk_root: String,
  /// `(static prefix, pattern)` pairs as `PathWithGlob` splits them.
  pub positive: Vec<(String, String)>,
  pub negated: Vec<(String, String)>,
  pub exhaustive: bool,
  pub case_sensitive: bool,
  /// The files the walk matched.
  pub matched: Vec<String>,
}
