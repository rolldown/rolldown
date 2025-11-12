use crate::fs_event::FsEventResult;

pub trait FsEventHandler: Send + 'static {
  /// Handles an event.
  fn handle_event(&mut self, event: FsEventResult);
}
