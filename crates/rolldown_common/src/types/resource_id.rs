use std::{
  borrow::Cow,
  ffi::OsStr,
  path::{Path, PathBuf},
  sync::Arc,
};

use regex::Regex;
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

  pub fn representative_name(&self) -> Cow<str> {
    representative_name(&self.0)
  }
}

static VALID_RE: once_cell::sync::Lazy<Regex> =
  once_cell::sync::Lazy::new(|| Regex::new(r"[^a-zA-Z0-9_$]").unwrap());

fn ensure_valid_identifier(s: Cow<str>) -> Cow<str> {
  match s {
    Cow::Borrowed(str) => VALID_RE.replace_all(str, "_"),
    Cow::Owned(owned_str) => VALID_RE.replace_all(&owned_str, "_").into_owned().into(),
  }
}

// This doesn't ensure uniqueness, but should be valid as a JS identifier.
pub fn representative_name(str: &str) -> Cow<str> {
  let path = Path::new(str);
  let mut unique_name =
    path.file_stem().map_or_else(|| Cow::Borrowed(str), |stem| stem.to_string_lossy());

  if unique_name == "index" {
    if let Some(unique_name_of_parent_dir) =
      path.parent().and_then(Path::file_stem).and_then(OsStr::to_str)
    {
      unique_name = Cow::Owned([unique_name_of_parent_dir, "_index"].concat());
    }
  }

  ensure_valid_identifier(unique_name)
}

#[test]
fn test_ensure_valid_identifier() {
  assert_eq!(ensure_valid_identifier("foo".into()), "foo");
  assert_eq!(ensure_valid_identifier("$foo$".into()), "$foo$");
  assert_eq!(ensure_valid_identifier("react-dom".into()), "react_dom");
}

pub fn stabilize_resource_id(resource_id: &str, cwd: &Path) -> String {
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
