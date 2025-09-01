use crate::event::EventResult;

pub trait EventHandler: Send + 'static {
  /// Handles an event.
  fn handle_event(&mut self, event: EventResult);
}
