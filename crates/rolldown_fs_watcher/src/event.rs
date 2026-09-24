use std::path::PathBuf;

use notify::Error as NotifyError;
use rolldown_common::WatcherChangeKind;

pub type FsEventResult = Result<Vec<FsEvent>, Vec<NotifyError>>;

/// A change of a file, translated from the events of the notify backend.
///
/// See `internal-docs/watch-mode/implementation.md` ("Notify Event Mapping").
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
  /// Handles a batch of events. The batch is never empty.
  fn handle_event(&mut self, event: FsEventResult);
}
