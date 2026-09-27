//! `watch.exclude` as notify's ignore filter. See internal-docs/watch-mode/implementation.md.

use std::path::Path;

use globstar::Glob;
use notify::EntryKind;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::{
  js_regex::HybridRegex,
  pattern_filter::{StringOrRegex, get_matcher_string, normalize_path},
};

/// An ignored path is never watched or reported, nor is anything below an ignored directory.
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
    self.glob.as_ref().is_some_and(|glob| {
      let path = path.as_bytes();
      match kind {
        EntryKind::File => glob.is_match(path),
        EntryKind::Dir => glob.match_dir(path).is_match(),
        EntryKind::Unknown => glob.is_match(path) || glob.match_dir(path).is_match(),
      }
    }) || self.regexes.iter().any(|regex| regex.matches(&path))
  }

  /// Looks up the kind of `path` on disk.
  pub fn is_path_ignored(&self, path: &Path) -> bool {
    let kind = path.metadata().map_or(EntryKind::Unknown, |metadata| metadata.file_type().into());
    self.is_ignored(path, kind)
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
    for kind in [EntryKind::File, EntryKind::Dir, EntryKind::Unknown] {
      assert!(filter.is_ignored(Path::new("/project/node_modules/pkg"), kind));
      assert!(filter.is_ignored(Path::new("/project/src/node_modules/pkg/index.js"), kind));
      assert!(filter.is_ignored(Path::new("/project/debug.log"), kind));
      assert!(!filter.is_ignored(Path::new("/project/src/debug.log"), kind));
      assert!(!filter.is_ignored(Path::new("/project/src/index.js"), kind));
      assert!(!filter.is_ignored(Path::new("/other/debug.log"), kind));
    }
  }

  #[test]
  fn directories() {
    let filter = filter(&[glob("src/**")]);
    // `**` needs at least one segment, so the directory itself is not ignored
    assert!(!filter.is_ignored(Path::new("/project/src"), EntryKind::Dir));
    assert!(filter.is_ignored(Path::new("/project/src/lib"), EntryKind::Dir));
    assert!(filter.is_ignored(Path::new("/project/src/lib"), EntryKind::Unknown));
    assert!(!filter.is_ignored(Path::new("/project/tests"), EntryKind::Dir));
  }

  #[test]
  fn regexes() {
    let filter = filter(&[regex("node_modules")]);
    assert!(filter.is_ignored(Path::new("/project/node_modules"), EntryKind::Dir));
    assert!(!filter.is_ignored(Path::new("/project/src/index.js"), EntryKind::File));
  }

  #[test]
  fn invalid_glob_is_an_error() {
    let error = IgnoreFilter::new(Some(&[glob("src/[")]), Path::new("/project"))
      .expect_err("an unterminated class is an error");
    assert!(format!("{error:?}").contains("src/["), "{error:?}");
  }
}
