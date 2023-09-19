use std::{fmt::Debug, hash::Hash, path::Path, sync::Arc};

use sugar_path::SugarPath;

use crate::RawPath;

struct Inner {
  path: RawPath,
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
  pub fn new(path: RawPath, cwd: impl AsRef<Path>) -> Self {
    let pretty = if Path::new(path.as_str()).is_absolute() {
      Path::new(path.as_str())
        .relative(cwd.as_ref())
        .into_os_string()
        .into_string()
        .unwrap()
    } else {
      path.to_string()
    };

    Self(Inner { path, pretty }.into())
  }

  pub fn prettify(&self) -> &str {
    &self.0.pretty
  }

  #[allow(clippy::needless_return)]
  pub fn generate_unique_name(&self) -> String {
    let path = Path::new(self.0.path.as_str());
    let unique_name = path.file_stem().unwrap().to_str().unwrap();
    if unique_name == "index" {
      if let Some(unique_name_of_parent_dir) = path
        .parent()
        .and_then(|p| p.file_stem())
        .and_then(|p| p.to_str())
      {
        return [unique_name_of_parent_dir, "_index"].concat();
      }
    }
    // TODO: ensure valid identifier
    return unique_name.to_string();
  }
}

impl AsRef<str> for ResourceId {
  fn as_ref(&self) -> &str {
    self.0.path.as_str()
  }
}
