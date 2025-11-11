use rolldown_watcher::EventHandler;

use crate::dev::{type_aliases::CoordinatorSender, types::coordinator_msg::CoordinatorMsg};

pub struct WatcherEventHandler {
  pub coordinator_tx: CoordinatorSender,
}
impl EventHandler for WatcherEventHandler {
  fn handle_event(&mut self, event: rolldown_watcher::FileChangeResult) {
    self.coordinator_tx.send(CoordinatorMsg::WatchEvent(event)).expect(
      "Coordinator channel closed while sending file change event - coordinator terminated unexpectedly"
    );
  }
}
