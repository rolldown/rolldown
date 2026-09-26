use std::borrow::Cow;

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

impl GlobMatcher {
  /// `file` is a slash-normalized absolute path.
  pub fn matches(&self, file: &str) -> bool {
    let Some(relative) = strip_dir_prefix(file, &self.walk_root) else {
      return false;
    };

    // Only the segments below the root: the walk's `filter_entry` exempts the root itself.
    if !self.exhaustive && relative.split('/').any(is_pruned_segment) {
      return false;
    }

    let file = self.fold(file);
    let matches_rule = |(prefix, glob): &(String, String)| {
      let prefix = self.fold(prefix);
      let glob = self.fold(glob);
      (*file).strip_prefix(&*prefix).is_some_and(|rest| fast_glob::glob_match(&*glob, rest))
    };
    !self.negated.iter().any(matches_rule) && self.positive.iter().any(matches_rule)
  }

  /// Whether `path` is a static glob prefix or an ancestor of one, the only signal available
  /// while the prefix itself does not exist.
  pub fn touches_base(&self, path: &str) -> bool {
    self.positive.iter().any(|(prefix, _)| {
      prefix.strip_prefix(path).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
  }

  /// Whether a file inside the directory `dir` could join the result set. The caller ensures
  /// `dir` really is a directory.
  pub fn may_gain_matches_below(&self, dir: &str) -> bool {
    let Some(relative) = strip_dir_prefix(dir, &self.walk_root) else {
      return false;
    };
    if !self.exhaustive && relative.split('/').any(is_pruned_segment) {
      return false;
    }
    // `strip_dir_prefix` rejects `dir == prefix`, which is `touches_base`'s business instead.
    // A pattern reaches below `dir` only past a second separator: the first one is its anchor.
    self.positive.iter().any(|(prefix, glob)| {
      (glob.trim_start_matches('/').contains('/') || glob.contains("**"))
        && strip_dir_prefix(dir, prefix).is_some()
    })
  }

  /// `fast_glob` has no case-insensitive flag, so folding lowercases both sides, same as the walk.
  fn fold<'a>(&self, path: &'a str) -> Cow<'a, str> {
    if self.case_sensitive { Cow::Borrowed(path) } else { Cow::Owned(path.to_lowercase()) }
  }
}

/// Compares on separator boundaries, so `/a/bc.js` is not read as living inside `/a/b`.
fn strip_dir_prefix<'a>(file: &'a str, dir: &str) -> Option<&'a str> {
  let rest = file.strip_prefix(dir)?;
  // Only a filesystem root (`/`, `C:/`) keeps a trailing separator.
  if dir.ends_with('/') { Some(rest) } else { rest.strip_prefix('/') }
}

fn is_pruned_segment(segment: &str) -> bool {
  segment.starts_with('.') || segment == "node_modules"
}
