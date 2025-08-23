use std::time::Instant;

use notify::{Error as NotifyError, Event as NotifyEvent};

pub type EventResult = Result<Vec<Event>, Vec<NotifyError>>;

/// A debounced event is emitted after a short delay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
  /// The original event.
  pub detail: NotifyEvent,

  /// The time at which the event occurred.
  pub time: Instant,
}
