use std::path::PathBuf;

use rolldown_common::{WatchPath, WatcherChangeKind};

/// A file change collected during debouncing, consumed by the rebuild sequence.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FileChangeEvent {
  pub path: WatchPath,
  pub kind: WatcherChangeKind,
}

impl FileChangeEvent {
  pub fn new(path: impl Into<PathBuf>, kind: WatcherChangeKind) -> Self {
    Self { path: WatchPath::from_absolute(path), kind }
  }
}
