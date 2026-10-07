//! `watch.exclude` as notify's ignore filter. See internal-docs/watch-mode/implementation.md.

use std::path::Path;

use globstar::Glob;
use notify::EntryKind;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::{
  js_regex::HybridRegex,
  pattern_filter::{StringOrRegex, get_matcher_string, normalize_path},
};

/// An ignored path is never watched or reported. A file is ignored if it matches, a directory
/// if everything below it matches, a path of unknown kind if either holds.
#[derive(Debug, Clone)]
pub struct IgnoreFilter {
  glob: Option<Glob>,
  regexes: Vec<HybridRegex>,
}

impl IgnoreFilter {
  /// `None` when nothing is ignored.
  pub fn new(patterns: Option<&[StringOrRegex]>, cwd: &Path) -> BuildResult<Option<Self>> {
    let Some(patterns) = patterns.filter(|patterns| !patterns.is_empty()) else {
      return Ok(None);
    };
    let cwd = cwd.to_string_lossy();
    let mut globs = Vec::new();
    let mut regexes = Vec::new();
    for pattern in patterns {
      match pattern {
        StringOrRegex::String(glob) => globs.push(get_matcher_string(glob, &cwd).into_owned()),
        StringOrRegex::Regex(regex) => regexes.push(regex.clone()),
      }
    }
    let glob = match globs.as_slice() {
      [] => None,
      globs => Some(Glob::union(globs).map_err(|error| {
        BuildDiagnostic::bundler_initialize_error(
          format!("Invalid watch patterns {globs:?}: {error}"),
          None,
        )
      })?),
    };
    Ok(Some(Self { glob, regexes }))
  }

  pub fn is_ignored(&self, path: &Path, kind: EntryKind) -> bool {
    let path = path.to_string_lossy();
    let path = normalize_path(&path);
    match kind {
      EntryKind::File => self.matches(&path),
      EntryKind::Dir => self.matches_all_below(&path),
      EntryKind::Unknown => self.matches(&path) || self.matches_all_below(&path),
    }
  }

  /// Looks up the kind of `path` on disk.
  pub fn is_path_ignored(&self, path: &Path) -> bool {
    let kind = path.metadata().map_or(EntryKind::Unknown, |metadata| metadata.file_type().into());
    self.is_ignored(path, kind)
  }

  fn matches(&self, path: &str) -> bool {
    self.glob.as_ref().is_some_and(|glob| glob.is_match(path.as_bytes()))
      || self.regexes.iter().any(|regex| regex.matches(path))
  }

  /// A regex cannot tell whether it matches everything below a directory.
  fn matches_all_below(&self, dir: &str) -> bool {
    self.glob.as_ref().is_some_and(|glob| glob.match_dir(dir.as_bytes()).matches_all_below())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn glob(pattern: &str) -> StringOrRegex {
    StringOrRegex::String(pattern.to_string())
  }

  fn regex(pattern: &str) -> StringOrRegex {
    StringOrRegex::Regex(HybridRegex::new(pattern).unwrap())
  }

  fn filter(patterns: &[StringOrRegex]) -> IgnoreFilter {
    IgnoreFilter::new(Some(patterns), Path::new("/project")).unwrap().unwrap()
  }

  #[test]
  fn nothing_ignored() {
    assert!(IgnoreFilter::new(None, Path::new("/project")).unwrap().is_none());
    assert!(IgnoreFilter::new(Some(&[]), Path::new("/project")).unwrap().is_none());
  }

  #[test]
  fn globs_are_resolved_against_cwd() {
    let filter = filter(&[glob("**/node_modules/**"), glob("*.log")]);
    assert!(
      filter.is_ignored(Path::new("/project/src/node_modules/pkg/index.js"), EntryKind::File)
    );
    assert!(filter.is_ignored(Path::new("/project/debug.log"), EntryKind::File));
    assert!(!filter.is_ignored(Path::new("/project/src/debug.log"), EntryKind::File));
    assert!(!filter.is_ignored(Path::new("/other/debug.log"), EntryKind::File));
  }

  #[test]
  fn kinds() {
    let filter = filter(&[glob("src/**"), glob("**/*.log"), regex("node_modules")]);
    for (path, file, dir) in [
      ("/project/src", false, true),
      ("/project/src/lib", true, true),
      ("/project/foo.log", true, false),
      ("/project/foo.log/a.js", false, false),
      ("/project/node_modules", true, false),
    ] {
      let path = Path::new(path);
      assert_eq!(filter.is_ignored(path, EntryKind::File), file, "{path:?} as a file");
      assert_eq!(filter.is_ignored(path, EntryKind::Dir), dir, "{path:?} as a directory");
      assert_eq!(filter.is_ignored(path, EntryKind::Unknown), file || dir, "{path:?}");
    }
  }

  #[test]
  fn invalid_glob_is_an_error() {
    let error = IgnoreFilter::new(Some(&[glob("src/[")]), Path::new("/project"))
      .expect_err("an unterminated class is an error");
    assert!(format!("{error:?}").contains("src/["), "{error:?}");
  }
}
