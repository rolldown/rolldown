use std::{fmt::Debug, path::Path};

use sugar_path::SugarPath;

use crate::FilePath;

#[derive(Clone, Debug)]
pub struct ResourceId(FilePath);

impl ResourceId {
  pub fn new(path: FilePath) -> Self {
    Self(path)
  }

  pub fn expect_file(&self) -> &FilePath {
    &self.0
  }

  pub fn prettify(&self, cwd: impl AsRef<Path>) -> String {
    let pretty = if Path::new(self.0.as_str()).is_absolute() {
      Path::new(self.0.as_str())
        .relative(cwd.as_ref())
        .into_os_string()
        .into_string()
        .expect("should be valid utf8")
    } else {
      self.0.to_string()
    };
    // remove \0
    pretty.replace('\0', "")
  }
}
