use std::{
  borrow::Cow,
  ffi::OsStr,
  path::{Component, Path},
  sync::Arc,
};

use regex::Regex;
use sugar_path::{AsPath, SugarPath};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct FilePath(Arc<str>);

impl FilePath {
  pub fn new(value: impl Into<String>) -> Self {
    Self(value.into().into())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl AsRef<str> for FilePath {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl std::ops::Deref for FilePath {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<String> for FilePath {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl FilePath {
  pub fn unique(&self, root: impl AsRef<Path>) -> String {
    let path = self.0.as_path();
    let mut relative = path.relative(root);
    let ext = relative.extension().and_then(OsStr::to_str).unwrap_or("").to_string();
    relative.set_extension("");

    let mut name = relative
      .components()
      .filter(|com| matches!(com, Component::Normal(_)))
      .filter_map(|seg| seg.as_os_str().to_str())
      .flat_map(|seg| seg.split('.'))
      .collect::<Vec<_>>()
      .join("_");

    if !ext.is_empty() {
      name.push('_');
      name.push_str(&ext);
    }
    name
  }

  pub fn prettify(&self, cwd: impl AsRef<Path>) -> String {
    let pretty = if Path::new(self.0.as_ref()).is_absolute() {
      Path::new(self.0.as_ref())
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

  // This doesn't ensure uniqueness, but should be valid as a JS identifier.
  pub fn representative_name(&self) -> Cow<str> {
    let path = Path::new(self.0.as_ref());
    let mut unique_name = Cow::Borrowed(
      path.file_stem().expect("should have file_stem").to_str().expect("should be valid utf8"),
    );

    if unique_name == "index" {
      if let Some(unique_name_of_parent_dir) =
        path.parent().and_then(Path::file_stem).and_then(OsStr::to_str)
      {
        unique_name = Cow::Owned([unique_name_of_parent_dir, "_index"].concat());
      }
    }

    ensure_valid_identifier(unique_name)
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

#[test]
fn test_ensure_valid_identifier() {
  assert_eq!(ensure_valid_identifier("foo".into()), "foo");
  assert_eq!(ensure_valid_identifier("$foo$".into()), "$foo$");
  assert_eq!(ensure_valid_identifier("react-dom".into()), "react_dom");
}

#[test]
fn test() {
  let cwd = "/projects/foo".to_string();
  let p = FilePath::new("/projects/foo/src/index.ts".to_string());
  assert_eq!(p.unique(&cwd), "src_index_ts");
  let p = FilePath::new("/projects/foo/src/index.module.css".to_string());
  assert_eq!(p.unique(&cwd), "src_index_module_css");
  // FIXME: "/projects/bar.ts" should not have the same result with "/bar.ts"
  let p = FilePath::new("/projects/bar.ts".to_string());
  assert_eq!(p.unique(&cwd), "bar_ts");
  let p = FilePath::new("/bar.ts".to_string());
  assert_eq!(p.unique(&cwd), "bar_ts");
}
