use std::path::Path;

use crate::FilePath;
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
    let pretty = pretty.replace('\0', "");

    if self.ignored {
      format!("(ignored) {pretty}")
    } else {
      pretty
    }
  }
}
