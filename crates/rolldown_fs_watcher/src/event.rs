use std::path::PathBuf;

use notify::Error as NotifyError;
use rolldown_common::WatcherChangeKind;

pub type FsEventResult = Result<Vec<FsEvent>, Vec<NotifyError>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEvent {
  pub path: PathBuf,
  pub kind: WatcherChangeKind,
}

impl FsEvent {
  pub fn new(path: PathBuf, kind: WatcherChangeKind) -> Self {
    Self { path, kind }
  }
}

pub trait FsEventHandler: Send + 'static {
  /// Never called with an empty batch.
  fn handle_event(&mut self, event: FsEventResult);
}
