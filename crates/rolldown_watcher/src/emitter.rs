use crate::event::WatcherEvent;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Default channel capacity for the event emitter
const DEFAULT_CHANNEL_CAPACITY: usize = 64;

/// Event emitter using broadcast channel for multi-subscriber support
#[derive(Debug)]
pub struct WatcherEmitter {
  tx: broadcast::Sender<WatcherEvent>,
}

impl WatcherEmitter {
  /// Create a new emitter with default capacity
  pub fn new() -> Self {
    Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
  }

  /// Create a new emitter with specified capacity
  pub fn with_capacity(capacity: usize) -> Self {
    let (tx, _) = broadcast::channel(capacity);
    Self { tx }
  }

  /// Emit an event to all subscribers
  ///
  /// Returns the number of receivers that received the event.
  /// If no receivers are listening, this is not an error - the event is simply dropped.
  pub fn emit(&self, event: WatcherEvent) -> usize {
    // send() returns Err if there are no receivers, which is fine
    self.tx.send(event).unwrap_or(0)
  }

  /// Subscribe to receive events
  ///
  /// Returns a receiver that will receive all events emitted after this call.
  pub fn subscribe(&self) -> broadcast::Receiver<WatcherEvent> {
    self.tx.subscribe()
  }

  /// Get the number of active receivers
  pub fn receiver_count(&self) -> usize {
    self.tx.receiver_count()
  }
}

impl Default for WatcherEmitter {
  fn default() -> Self {
    Self::new()
  }
}

/// Shared reference to a watcher emitter
pub type SharedWatcherEmitter = Arc<WatcherEmitter>;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::event::BundleEvent;

  #[tokio::test]
  async fn test_emit_without_subscribers() {
    let emitter = WatcherEmitter::new();
    // Should not panic or error
    let count = emitter.emit(WatcherEvent::Close);
    assert_eq!(count, 0);
  }

  #[tokio::test]
  async fn test_emit_with_subscriber() {
    let emitter = WatcherEmitter::new();
    let mut rx = emitter.subscribe();

    let count = emitter.emit(WatcherEvent::Event(BundleEvent::Start));
    assert_eq!(count, 1);

    let event = rx.recv().await.unwrap();
    assert!(matches!(event, WatcherEvent::Event(BundleEvent::Start)));
  }

  #[tokio::test]
  async fn test_multiple_subscribers() {
    let emitter = WatcherEmitter::new();
    let mut rx1 = emitter.subscribe();
    let mut rx2 = emitter.subscribe();

    let count = emitter.emit(WatcherEvent::Close);
    assert_eq!(count, 2);

    let event1 = rx1.recv().await.unwrap();
    let event2 = rx2.recv().await.unwrap();

    assert!(matches!(event1, WatcherEvent::Close));
    assert!(matches!(event2, WatcherEvent::Close));
  }
}
