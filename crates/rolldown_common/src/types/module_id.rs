use std::path::{Path, PathBuf};

use arcstr::ArcStr;
use rolldown_std_utils::PathExt;
use sugar_path::SugarPath;

/// `ModuleId` is the unique string identifier for each module.
/// - It will be used to identify the module in the whole bundle.
/// - Users could stored the `ModuleId` to track the module in different stages/hooks.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Default)]
pub struct ModuleId {
  // Id that rolldown uses to call `read_to_string` or `read` to get the content of the module.
  resource_id: ArcStr,
}

impl ModuleId {
  #[inline]
  pub fn new(value: impl Into<ArcStr>) -> Self {
    Self::new_arc_str(value.into())
  }

  #[inline]
  pub const fn new_arc_str(resource_id: ArcStr) -> Self {
    Self { resource_id }
  }

  pub fn resource_id(&self) -> &ArcStr {
    &self.resource_id
  }

  pub fn stabilize(&self, cwd: &Path) -> String {
    stabilize_module_id(&self.resource_id, cwd)
  }
}

impl AsRef<str> for ModuleId {
  fn as_ref(&self) -> &str {
    &self.resource_id
  }
}

impl std::ops::Deref for ModuleId {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.resource_id
  }
}

impl From<&str> for ModuleId {
  fn from(value: &str) -> Self {
    Self::new(value)
  }
}

impl From<String> for ModuleId {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl From<ArcStr> for ModuleId {
  fn from(value: ArcStr) -> Self {
    Self::new(value)
  }
}

impl ModuleId {
  pub fn relative_path(&self, root: impl AsRef<Path>) -> PathBuf {
    let path = self.resource_id.as_path();
    path.relative(root)
  }
}

pub fn stabilize_module_id(module_id: &str, cwd: &Path) -> String {
  if module_id.as_path().is_absolute() {
    module_id.relative(cwd).as_path().expect_to_slash()
  } else if module_id.starts_with('\0') {
    // handle virtual modules
    module_id.replace('\0', "\\0")
  } else {
    module_id.to_string()
  }
}

#[test]
fn test_stabilize_module_id() {
  let cwd = std::env::current_dir().unwrap();
  // absolute path
  assert_eq!(
    stabilize_module_id(cwd.join("src").join("main.js").expect_to_str(), &cwd),
    "src/main.js"
  );
  assert_eq!(
    stabilize_module_id(cwd.join("..").join("src").join("main.js").expect_to_str(), &cwd),
    "../src/main.js"
  );

  // non-path specifier
  assert_eq!(stabilize_module_id("fs", &cwd), "fs");
  assert_eq!(
    stabilize_module_id("https://deno.land/x/oak/mod.ts", &cwd),
    "https://deno.land/x/oak/mod.ts"
  );

  // virtual module
  assert_eq!(stabilize_module_id("\0foo", &cwd), "\\0foo");
}
