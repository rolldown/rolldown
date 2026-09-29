use std::borrow::Cow;

use arcstr::ArcStr;
use rolldown_common::WatcherChangeKind;
use rolldown_plugin::HookHotUpdateArgs;

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

  /// `args.modules` and the modules whose glob result `args` changes, if there are any.
  pub(crate) fn add_glob_owners(&self, args: &HookHotUpdateArgs) -> Option<Vec<ArcStr>> {
    let changes_result: fn(&GlobMatcher, &str) -> bool = match args.kind {
      WatcherChangeKind::Create => GlobMatcher::gains,
      WatcherChangeKind::Delete => {
        // A module deleted with `args.file` cannot be fetched again.
        self.glob_matchers.retain(|id, _| relative_to(file_of(id), &args.file).is_none());
        GlobMatcher::loses
      }
      // A content edit cannot change which files a glob matches.
      WatcherChangeKind::Update => return None,
    };

    let mut owners = self
      .glob_matchers
      .iter()
      .filter(|entry| {
        // The walk leaves out the module of the glob itself.
        entry.key() != &args.file
          && !args.modules.contains(entry.key())
          && entry.value().iter().any(|matcher| changes_result(matcher, &args.file))
      })
      .map(|entry| entry.key().clone())
      .collect::<Vec<_>>();
    if owners.is_empty() {
      return None;
    }
    // The map has no stable order.
    owners.sort_unstable();

    // Appending rather than replacing, like vite's `[...oldModules, ...modules]`
    let mut modules = args.modules.clone();
    modules.append(&mut owners);
    Some(modules)
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

impl GlobMatcher {
  /// Whether creating `file` adds it to the result. A file of the result is created again when
  /// it is saved by renaming a temporary file over it.
  pub fn gains(&self, file: &str) -> bool {
    self.matches(file) && !self.matched.iter().any(|matched| matched == file)
  }

  /// Whether deleting `path` removes files from the result. A deleted directory is reported
  /// without the files below it.
  pub fn loses(&self, path: &str) -> bool {
    self.matched.iter().any(|matched| relative_to(matched, path).is_some())
  }

  /// Decides like the walk in [`crate::utils::GlobImportVisit`]. `file` is a slash-normalized
  /// absolute path.
  fn matches(&self, file: &str) -> bool {
    let Some(relative) = relative_to(file, &self.walk_root) else {
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

  /// `fast_glob` has no case-insensitive flag, so folding lowercases both sides, same as the walk.
  fn fold<'a>(&self, path: &'a str) -> Cow<'a, str> {
    if self.case_sensitive { Cow::Borrowed(path) } else { Cow::Owned(path.to_lowercase()) }
  }
}

/// `path` relative to `dir`, if it is `dir` or below it. Compares on separator boundaries, so
/// `/a/bc.js` is not read as living inside `/a/b`.
fn relative_to<'a>(path: &'a str, dir: &str) -> Option<&'a str> {
  let rest = path.strip_prefix(dir)?;
  if rest.is_empty() || dir.ends_with('/') { Some(rest) } else { rest.strip_prefix('/') }
}

/// The module id without its query.
fn file_of(id: &str) -> &str {
  id.split_once('?').map_or(id, |(file, _)| file)
}

fn is_pruned_segment(segment: &str) -> bool {
  segment.starts_with('.') || segment == "node_modules"
}

#[cfg(test)]
mod tests {
  use arcstr::ArcStr;
  use rolldown_common::WatcherChangeKind::{self, Create, Delete, Update};
  use rolldown_plugin::HookHotUpdateArgs;

  use super::GlobMatcher;
  use crate::{ViteImportGlobPlugin, utils::PathWithGlob};

  fn split(glob: &str) -> (String, String) {
    PathWithGlob::new(glob.to_string(), glob).to_owned_parts()
  }

  fn matcher(walk_root: &str, positive: &[&str], negated: &[&str]) -> GlobMatcher {
    GlobMatcher {
      walk_root: walk_root.to_string(),
      positive: positive.iter().copied().map(split).collect(),
      negated: negated.iter().copied().map(split).collect(),
      exhaustive: false,
      case_sensitive: true,
      matched: Vec::new(),
    }
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
  fn matches_a_glob_that_starts_with_a_pattern() {
    // `**/index.js` written next to `/p/main.js`: the prefix keeps the separator.
    let parts = PathWithGlob::new("/p/**/index.js".to_string(), "**/index.js").to_owned_parts();
    assert_eq!(parts, ("/p/".to_string(), "**/index.js".to_string()));
    let m = GlobMatcher { positive: vec![parts], ..matcher("/p/", &[], &[]) };
    assert!(m.matches("/p/index.js"));
    assert!(m.matches("/p/src/index.js"));
    assert!(!m.matches("/p/src/main.js"));
  }

  #[test]
  fn matches_a_glob_without_a_pattern() {
    let m = matcher("/p/src/page.js", &["/p/src/page.js"], &[]);
    assert!(m.matches("/p/src/page.js"));
    assert!(!m.matches("/p/src/page.jsx"));
  }

  #[test]
  fn honors_negated_patterns() {
    let m = matcher("/p/src/pages", &["/p/src/pages/*.js"], &["/p/src/pages/*.test.js"]);
    assert!(m.matches("/p/src/pages/a.js"));
    assert!(!m.matches("/p/src/pages/a.test.js"));
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
    let m = GlobMatcher { exhaustive: true, ..matcher("/p/src", &["/p/src/**/*.js"], &[]) };
    assert!(m.matches("/p/src/.cache/a.js"));
    assert!(m.matches("/p/src/node_modules/dep/a.js"));
  }

  #[test]
  fn folds_case_when_case_sensitive_is_off() {
    let sensitive = matcher("/p/src", &["/p/src/*.JS"], &[]);
    assert!(!sensitive.matches("/p/src/a.js"));

    let insensitive = GlobMatcher { case_sensitive: false, ..sensitive };
    assert!(insensitive.matches("/p/src/a.js"));
  }

  #[test]
  fn gains_a_matching_file_that_is_not_in_the_result() {
    let m = GlobMatcher {
      matched: vec!["/p/src/pages/a.js".to_string()],
      ..matcher("/p/src/pages", &["/p/src/pages/*.js"], &[])
    };
    assert!(m.gains("/p/src/pages/b.js"));
    assert!(!m.gains("/p/src/pages/a.js"));
    assert!(!m.gains("/p/src/pages/notes.txt"));
  }

  #[test]
  fn loses_a_file_of_the_result_or_a_directory_above_one() {
    let m = GlobMatcher {
      matched: vec!["/p/src/pages/a.js".to_string(), "/p/src/pages/deep/b.js".to_string()],
      ..matcher("/p/src/pages", &["/p/src/pages/**/*.js"], &[])
    };
    assert!(m.loses("/p/src/pages/a.js"));
    assert!(m.loses("/p/src/pages/deep"));
    assert!(m.loses("/p/src"));
    // Matches the pattern, but was never part of the result.
    assert!(!m.loses("/p/src/pages/c.js"));
    assert!(!m.loses("/p/src/pages/de"));
  }

  fn plugin(owners: &[&str]) -> ViteImportGlobPlugin {
    let plugin = ViteImportGlobPlugin::default();
    for owner in owners {
      let matcher = GlobMatcher {
        matched: vec!["/p/pages/a.js".to_string()],
        ..matcher("/p/pages", &["/p/pages/*.js"], &[])
      };
      plugin.set_globs(owner, vec![matcher]);
    }
    plugin
  }

  fn add_glob_owners(
    plugin: &ViteImportGlobPlugin,
    kind: WatcherChangeKind,
    file: &str,
    modules: &[&str],
  ) -> Option<Vec<ArcStr>> {
    let modules = modules.iter().copied().map(ArcStr::from).collect();
    plugin.add_glob_owners(&HookHotUpdateArgs { kind, file: file.into(), modules })
  }

  #[test]
  fn adds_the_owners_in_a_stable_order() {
    let plugin = plugin(&["/p/b.js", "/p/a.js"]);
    assert_eq!(
      add_glob_owners(&plugin, Create, "/p/pages/b.js", &["/p/pages/b.js"]).unwrap(),
      ["/p/pages/b.js", "/p/a.js", "/p/b.js"]
    );
    assert_eq!(add_glob_owners(&plugin, Delete, "/p/pages", &[]).unwrap(), ["/p/a.js", "/p/b.js"]);
  }

  #[test]
  fn declines_what_does_not_change_the_result() {
    let plugin = plugin(&["/p/main.js"]);
    assert_eq!(add_glob_owners(&plugin, Update, "/p/pages/a.js", &["/p/pages/a.js"]), None);
    assert_eq!(add_glob_owners(&plugin, Create, "/p/pages/a.js", &[]), None);
    assert_eq!(add_glob_owners(&plugin, Create, "/p/pages/notes.txt", &[]), None);
    assert_eq!(add_glob_owners(&plugin, Delete, "/p/pages/b.js", &[]), None);
    // The owner is part of the update already.
    assert_eq!(add_glob_owners(&plugin, Create, "/p/pages/b.js", &["/p/main.js"]), None);
  }

  #[test]
  fn leaves_out_the_module_of_the_glob_itself() {
    let plugin = plugin(&["/p/pages/main.js"]);
    assert_eq!(add_glob_owners(&plugin, Create, "/p/pages/main.js", &[]), None);
    assert!(add_glob_owners(&plugin, Create, "/p/pages/b.js", &[]).is_some());
  }

  #[test]
  fn forgets_the_owners_deleted_with_the_path() {
    let plugin = plugin(&["/p/pages/main.js", "/p/app.vue?vue&type=script", "/p/main.js"]);
    assert_eq!(add_glob_owners(&plugin, Delete, "/p/app.vue", &[]), None);
    assert_eq!(add_glob_owners(&plugin, Delete, "/p/pages", &[]).unwrap(), ["/p/main.js"]);
    let mut owners = plugin.glob_matchers.iter().map(|e| e.key().clone()).collect::<Vec<_>>();
    owners.sort_unstable();
    assert_eq!(owners, ["/p/main.js"]);
  }
}
