use std::{path::Path, sync::Arc};

use sugar_path::{AsPath, SugarPath};

#[derive(Debug, Clone)]
pub struct ResolvedPath {
  pub path: Arc<str>,
  pub ignored: bool,
}

impl From<String> for ResolvedPath {
  fn from(value: String) -> Self {
    Self { path: value.into(), ignored: false }
  }
}

impl ResolvedPath {
  /// Created a pretty string representation of the path. The path
  /// 1. doesn't guarantee to be unique
  /// 2. relative to the cwd, so it could show stable path across different machines
  pub fn prettify(&self, cwd: impl AsRef<Path>) -> String {
    let path = self.path.as_path();
    let pretty = if path.is_absolute() {
      path.relative(cwd.as_ref()).to_string_lossy().to_string()
    } else {
      path.to_string_lossy().to_string()
    };
    // remove \0
    let mut pretty = pretty.replace('\0', "");
    if cfg!(target_os = "windows") {
      // TODO: remove this after https://github.com/hyf0/sugar_path/issues/18 is solved
      // To use snapshots across platforms in testing, replace all backslashes with slashes
      pretty = pretty.replace('\\', "/");
    }
    if self.ignored {
      format!("(ignored) {pretty}")
    } else {
      pretty
    }
  }
}

#[test]
fn test() {
  let mut current_dir = std::env::current_dir().unwrap().display().to_string();
  current_dir.push_str("/resolved_path.rs");
  let mut from_test = ResolvedPath::from(current_dir);

  let prettify_res = from_test.prettify(Path::new("../"));

  assert_eq!(prettify_res, "rolldown_resolver/resolved_path.rs");

  from_test.ignored = true;

  let ignore_prettify_res = from_test.prettify(Path::new("./"));

  assert_eq!(ignore_prettify_res, "(ignored) resolved_path.rs");
}
