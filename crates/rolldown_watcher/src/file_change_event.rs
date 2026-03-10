use rolldown_common::WatcherChangeKind;

/// A file change collected during debouncing, consumed by the rebuild sequence.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FileChangeEvent {
  pub path: String,
  pub kind: WatcherChangeKind,
}

impl FileChangeEvent {
  pub fn new(path: String, kind: WatcherChangeKind) -> Self {
    Self { path, kind }
  }
}
