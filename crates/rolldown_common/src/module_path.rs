use std::{ffi::OsStr, fmt::Debug, hash::Hash, path::Path, sync::Arc};

use sugar_path::SugarPath;

use crate::FilePath;

struct Inner {
  path: FilePath,
  pretty: String,
}

#[derive(Clone)]
pub struct ResourceId(Arc<Inner>);

impl Hash for ResourceId {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.0.path.hash(state);
  }
}

impl PartialEq for ResourceId {
  fn eq(&self, other: &Self) -> bool {
    self.0.path.eq(&other.0.path)
  }
}
impl Eq for ResourceId {}

impl Debug for ResourceId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple(&self.0.pretty).finish()
  }
}

impl ResourceId {
  pub fn new(path: FilePath, cwd: impl AsRef<Path>) -> Self {
    let pretty = if Path::new(path.as_str()).is_absolute() {
      Path::new(path.as_str())
        .relative(cwd.as_ref())
        .into_os_string()
        .into_string()
        .expect("should be valid utf8")
    } else {
      path.to_string()
    };
    // remove \0
    let pretty = pretty.replace('\0', "");

    Self(Inner { path, pretty }.into())
  }

  pub fn prettify(&self) -> &str {
    &self.0.pretty
  }

  #[allow(clippy::needless_return)]
  pub fn generate_unique_name(&self) -> String {
    let path = Path::new(self.0.path.as_str());
    let unique_name =
      path.file_stem().expect("should have file_stem").to_str().expect("should be valid utf8");
    if unique_name == "index" {
      if let Some(unique_name_of_parent_dir) =
        path.parent().and_then(Path::file_stem).and_then(OsStr::to_str)
      {
        return [unique_name_of_parent_dir, "_index"].concat();
      }
    }
    return ensure_valid_identifier(unique_name);
  }
}

impl AsRef<str> for ResourceId {
  fn as_ref(&self) -> &str {
    self.0.path.as_str()
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
