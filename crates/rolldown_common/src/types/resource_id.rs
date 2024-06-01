use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use rolldown_utils::path_ext::PathExt;
use sugar_path::SugarPath;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct ResourceId(Arc<str>);

impl ResourceId {
  pub fn new(value: impl Into<Arc<str>>) -> Self {
    Self(value.into())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  pub fn stabilize(&self, cwd: &Path) -> String {
    stabilize_resource_id(&self.0, cwd)
  }
}

impl AsRef<str> for ResourceId {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl std::ops::Deref for ResourceId {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<String> for ResourceId {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl From<Arc<str>> for ResourceId {
  fn from(value: Arc<str>) -> Self {
    Self(value)
  }
}

impl ResourceId {
  pub fn relative_path(&self, root: impl AsRef<Path>) -> PathBuf {
    let path = self.0.as_path();
    path.relative(root)
  }
}

pub(crate) fn stabilize_resource_id(resource_id: &str, cwd: &Path) -> String {
  if resource_id.as_path().is_absolute() {
    resource_id.relative(cwd).as_path().expect_to_slash()
  } else if resource_id.starts_with('\0') {
    // handle virtual modules
    resource_id.replace('\0', "\\0")
  } else {
    resource_id.to_string()
  }
}

#[test]
fn test_stabilize_resource_id() {
  let cwd = std::env::current_dir().unwrap();
  // absolute path
  assert_eq!(
    stabilize_resource_id(cwd.join("src").join("main.js").expect_to_str(), &cwd),
    "src/main.js"
  );
  assert_eq!(
    stabilize_resource_id(cwd.join("..").join("src").join("main.js").expect_to_str(), &cwd),
    "../src/main.js"
  );

  // non-path specifier
  assert_eq!(stabilize_resource_id("fs", &cwd), "fs");
  assert_eq!(
    stabilize_resource_id("https://deno.land/x/oak/mod.ts", &cwd),
    "https://deno.land/x/oak/mod.ts"
  );

  // virtual module
  assert_eq!(stabilize_resource_id("\0foo", &cwd), "\\0foo");
}
