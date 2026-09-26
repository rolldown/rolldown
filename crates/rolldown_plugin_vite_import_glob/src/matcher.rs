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
///
/// See `internal-docs/import-meta-glob/design.md`.
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

#[cfg(test)]
mod tests {
  use super::GlobMatcher;

  /// Splits an absolute glob the way `PathWithGlob` does: at the last separator before the first
  /// glob metacharacter, with that separator kept on the pattern side.
  fn split(glob: &str) -> (String, String) {
    let first_meta = glob.find(['*', '?', '[', '{', '\\']).unwrap_or(glob.len());
    let at = glob[..first_meta].rfind('/').expect("test globs are absolute");
    (glob[..at].to_string(), glob[at..].to_string())
  }

  fn matcher_with(
    walk_root: &str,
    positive: &[&str],
    negated: &[&str],
    exhaustive: bool,
    case_sensitive: bool,
  ) -> GlobMatcher {
    GlobMatcher {
      walk_root: walk_root.to_string(),
      positive: positive.iter().copied().map(split).collect(),
      negated: negated.iter().copied().map(split).collect(),
      exhaustive,
      case_sensitive,
    }
  }

  fn matcher(walk_root: &str, positive: &[&str], negated: &[&str]) -> GlobMatcher {
    matcher_with(walk_root, positive, negated, false, true)
  }

  #[test]
  fn matches_a_single_level_pattern() {
    let m = matcher("/p/src/pages", &["/p/src/pages/*.js"], &[]);
    assert!(m.matches("/p/src/pages/a.js"));
    assert!(!m.matches("/p/src/pages/a.ts"));
    // `*` does not cross a separator, and the sibling directory is outside the walk root.
    assert!(!m.matches("/p/src/pages/nested/a.js"));
    assert!(!m.matches("/p/src/main.js"));
  }

  #[test]
  fn rejects_paths_outside_the_walk_root_even_on_a_shared_string_prefix() {
    let m = matcher("/p/src/pages", &["/p/src/pages/*.js"], &[]);
    assert!(!m.matches("/p/src/pages-legacy/a.js"));
  }

  #[test]
  fn matches_a_globstar_pattern_across_levels() {
    let m = matcher("/p/src/pages", &["/p/src/pages/**/*.js"], &[]);
    assert!(m.matches("/p/src/pages/a.js"));
    assert!(m.matches("/p/src/pages/nested/deep/a.js"));
    assert!(!m.matches("/p/src/pages/nested/a.css"));
  }

  #[test]
  fn honors_negated_patterns() {
    let m = matcher("/p/src/pages", &["/p/src/pages/*.js"], &["/p/src/pages/*.test.js"]);
    assert!(m.matches("/p/src/pages/a.js"));
    assert!(!m.matches("/p/src/pages/a.test.js"));
  }

  #[test]
  fn requires_a_positive_hit() {
    // Only negated globs: the build-time walk yields nothing, so neither may the matcher.
    let m = matcher("/p/src", &[], &["/p/src/*.js"]);
    assert!(!m.matches("/p/src/a.js"));
    assert!(!m.matches("/p/src/b.ts"));
  }

  #[test]
  fn prunes_dot_and_node_modules_segments_below_the_walk_root() {
    let m = matcher("/p/src", &["/p/src/**/*.js"], &[]);
    assert!(m.matches("/p/src/a.js"));
    assert!(!m.matches("/p/src/.cache/a.js"));
    assert!(!m.matches("/p/src/.hidden.js"));
    assert!(!m.matches("/p/src/node_modules/dep/a.js"));
    // A dot segment inside the walk root is part of the root, not something the walk pruned.
    let m = matcher("/p/.storybook", &["/p/.storybook/*.js"], &[]);
    assert!(m.matches("/p/.storybook/a.js"));
  }

  #[test]
  fn exhaustive_keeps_dot_and_node_modules_segments() {
    let m = matcher_with("/p/src", &["/p/src/**/*.js"], &[], true, true);
    assert!(m.matches("/p/src/.cache/a.js"));
    assert!(m.matches("/p/src/node_modules/dep/a.js"));
  }

  #[test]
  fn folds_case_when_case_sensitive_is_off() {
    let sensitive = matcher_with("/p/src", &["/p/src/*.JS"], &[], false, true);
    assert!(!sensitive.matches("/p/src/a.js"));

    let insensitive = matcher_with("/p/src", &["/p/src/*.JS"], &[], false, false);
    assert!(insensitive.matches("/p/src/a.js"));
  }

  #[test]
  fn touches_base_covers_the_prefix_and_its_ancestors() {
    let m = matcher("/p/src", &["/p/src/pages/*.js", "/p/src/layouts/*.js"], &[]);
    assert!(m.touches_base("/p/src/pages"));
    assert!(m.touches_base("/p/src/layouts"));
    assert!(m.touches_base("/p/src"));
    assert!(m.touches_base("/p"));
    // Not on the path to any prefix.
    assert!(!m.touches_base("/p/src/pages/a.js"));
    assert!(!m.touches_base("/p/src/pages-legacy"));
    assert!(!m.touches_base("/other"));
  }

  #[test]
  fn may_gain_matches_below_only_for_patterns_that_reach_deeper() {
    let single = matcher("/p/src/pages", &["/p/src/pages/*.js"], &[]);
    // `*.js` can never match below `pages/`, so a new subdirectory changes nothing.
    assert!(!single.may_gain_matches_below("/p/src/pages/sub"));

    let deep = matcher("/p/src/pages", &["/p/src/pages/**/*.js"], &[]);
    assert!(deep.may_gain_matches_below("/p/src/pages/sub"));
    assert!(deep.may_gain_matches_below("/p/src/pages/sub/deeper"));
    // The base itself is `touches_base`'s job, and anything outside the walk root is irrelevant.
    assert!(!deep.may_gain_matches_below("/p/src/pages"));
    assert!(!deep.may_gain_matches_below("/p/src/other"));
    // Pruned directories are not descended into, so they cannot contribute either.
    assert!(!deep.may_gain_matches_below("/p/src/pages/.cache"));
    assert!(!deep.may_gain_matches_below("/p/src/pages/node_modules"));

    // A pattern spanning segments without a globstar counts too.
    let two = matcher("/p/src/pages", &["/p/src/pages/*/index.js"], &[]);
    assert!(two.may_gain_matches_below("/p/src/pages/sub"));
  }
}
