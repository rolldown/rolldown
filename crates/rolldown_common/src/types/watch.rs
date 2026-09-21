use std::{
  borrow::Borrow,
  fmt::Display,
  path::{Path, PathBuf},
  sync::Arc,
};

use rolldown_std_utils::{absolutize_path_buf, normalize_path_buf};

// See internal-docs/watch-mode/implementation.md ("Path Identity").
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WatchPath(Arc<Path>);

impl WatchPath {
  pub fn new(path: impl Into<PathBuf>, cwd: &Path) -> Self {
    let path = path.into();
    Self::from_absolute(cwd.join(path))
  }

  pub fn from_absolute(path: impl Into<PathBuf>) -> Self {
    let path = absolutize_path_buf(path.into());
    Self(Arc::from(normalize_path_buf(path)))
  }

  #[inline]
  pub fn as_path(&self) -> &Path {
    &self.0
  }
}

impl Borrow<Path> for WatchPath {
  fn borrow(&self) -> &Path {
    self.as_path()
  }
}

impl Display for WatchPath {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.as_path().display().fmt(f)
  }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum WatcherChangeKind {
  Create,
  Update,
  Delete,
}

impl Display for WatcherChangeKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      WatcherChangeKind::Create => write!(f, "create"),
      WatcherChangeKind::Update => write!(f, "update"),
      WatcherChangeKind::Delete => write!(f, "delete"),
    }
  }
}
