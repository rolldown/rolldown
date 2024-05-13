use std::{path::Path, sync::Arc};

use sugar_path::SugarPath;

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
    let mut pretty = if path.is_absolute() {
      path.relative(cwd.as_ref()).to_slash_lossy().to_string()
    } else {
      path.to_slash_lossy().to_string()
    };
    // remove \0
    pretty.retain(|c| c != '\0');
    if self.ignored {
      pretty.insert_str(0, "(ignored) ");
    }
    pretty
  }
}

#[test]
fn test() {
  let mut current_dir = std::env::current_dir().unwrap().display().to_string();
  current_dir.push_str("/resolved_path.rs");
  let mut from_test = ResolvedPath::from(current_dir);

  let prettify_res = from_test.prettify(Path::new("../"));

  assert_eq!(prettify_res, "rolldown_common/resolved_path.rs");

  from_test.ignored = true;

  let ignore_prettify_res = from_test.prettify(Path::new("./"));

  assert_eq!(ignore_prettify_res, "(ignored) resolved_path.rs");
}
