use crate::FilePath;
use std::env;
use std::path::Path;
use sugar_path::SugarPath;

#[derive(Debug, Clone)]
pub struct ResolvedPath {
  pub path: FilePath,
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
    let pretty = if Path::new(self.path.as_ref()).is_absolute() {
      Path::new(self.path.as_ref())
        .relative(cwd.as_ref())
        .into_os_string()
        .into_string()
        .expect("should be valid utf8")
    } else {
      self.path.to_string()
    };
    // remove \0
    let mut pretty = pretty.replace('\0', "");
    if cfg!(target_os = "windows") {
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
  let mut current_dir = env::current_dir().unwrap().display().to_string();
  current_dir.push_str("/resolved_path.rs");
  let mut from_test = ResolvedPath::from(current_dir);

  let prettify_res = from_test.prettify(Path::new("../"));

  assert_eq!(prettify_res, "rolldown_common/resolved_path.rs");

  from_test.ignored = true;

  let ignore_prettify_res = from_test.prettify(Path::new("./"));

  assert_eq!(ignore_prettify_res, "(ignored) resolved_path.rs");
}
