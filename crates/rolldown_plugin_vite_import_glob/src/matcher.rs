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
