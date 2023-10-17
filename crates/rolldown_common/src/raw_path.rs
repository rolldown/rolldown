use std::{
  ffi::OsStr,
  path::{Component, Path},
};

use sugar_path::{AsPath, SugarPath};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct RawPath(String);

impl std::ops::Deref for RawPath {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<String> for RawPath {
  fn from(value: String) -> Self {
    Self(value)
  }
}

impl RawPath {
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
}

#[test]
fn test() {
  let cwd = "/projects/foo".to_string();
  let p = RawPath("/projects/foo/src/index.ts".to_string());
  assert_eq!(p.unique(&cwd), "src_index_ts");
  let p = RawPath("/projects/foo/src/index.module.css".to_string());
  assert_eq!(p.unique(&cwd), "src_index_module_css");
  // FIXME: "/projects/bar.ts" should not have the same result with "/bar.ts"
  let p = RawPath("/projects/bar.ts".to_string());
  assert_eq!(p.unique(&cwd), "bar_ts");
  let p = RawPath("/bar.ts".to_string());
  assert_eq!(p.unique(&cwd), "bar_ts");
}
