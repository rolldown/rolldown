use std::time::Instant;

use notify::{Error as NotifyError, Event as NotifyEvent};

pub type FsEventResult = Result<Vec<FsEvent>, Vec<NotifyError>>;

/// A filesystem event emitted by a watcher backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEvent {
  /// The original event.
  pub detail: NotifyEvent,

  /// The time at which the event occurred.
  pub time: Instant,
}

pub trait FsEventHandler: Send + 'static {
  /// Handles an event.
  fn handle_event(&mut self, event: FsEventResult);
}
