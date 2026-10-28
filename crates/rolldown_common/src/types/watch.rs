use std::{borrow::Borrow, fmt::Display, path::Path, sync::Arc};

use rolldown_std_utils::normalize_path_buf;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct WatchPath(Arc<Path>);

impl WatchPath {
  pub fn new(path: impl AsRef<Path>, cwd: &Path) -> Self {
    Self(Arc::from(normalize_path_buf(cwd.join(path))))
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

#[cfg(all(test, not(windows)))]
mod tests {
  use super::*;

  #[test]
  fn watch_path() {
    let cwd = Path::new("/project");
    let watch_path = WatchPath::new("src/./pages/../content/", cwd);
    assert_eq!(watch_path.as_path(), Path::new("/project/src/content"));
    assert_eq!(WatchPath::new("/other/dir", cwd).as_path(), Path::new("/other/dir"));

    let watch_paths = rustc_hash::FxHashSet::from_iter([watch_path]);
    let is_watched =
      |path: &str| Path::new(path).ancestors().any(|ancestor| watch_paths.contains(ancestor));
    assert!(is_watched("/project/src/content/nested/index.js"));
    assert!(!is_watched("/project/src/content-other/index.js"));
  }
}
