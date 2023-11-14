use std::{
  ffi::OsStr,
  path::{Component, Path},
  sync::Arc,
};

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

  pub fn generate_unique_name(&self) -> String {
    let path = Path::new(self.0.as_ref());
    let unique_name =
      path.file_stem().expect("should have file_stem").to_str().expect("should be valid utf8");
    if unique_name == "index" {
      if let Some(unique_name_of_parent_dir) =
        path.parent().and_then(Path::file_stem).and_then(OsStr::to_str)
      {
        return [unique_name_of_parent_dir, "_index"].concat();
      }
    }
    ensure_valid_identifier(unique_name)
  }
}

fn ensure_valid_identifier(s: &str) -> String {
  let mut ident = String::new();
  let mut need_gap = false;
  for i in s.chars() {
    if i.is_ascii_alphabetic() || (i.is_ascii_digit() && !ident.is_empty()) {
      if need_gap {
        ident.push('_');
        need_gap = false;
      }
      ident.push(i);
    } else if !ident.is_empty() {
      need_gap = true;
    }
  }
  if ident.is_empty() {
    ident.push('_');
  }
  ident
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
