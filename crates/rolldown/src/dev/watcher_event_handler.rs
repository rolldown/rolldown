use rolldown_watcher::EventHandler;

use crate::dev::build_driver_service::{BuildChannelTx, BuildMessage};

pub struct WatcherEventHandler {
  pub service_tx: BuildChannelTx,
}
impl EventHandler for WatcherEventHandler {
  fn handle_event(&mut self, event: rolldown_watcher::FileChangeResult) {
    self.service_tx.send(BuildMessage::WatchEvent(event)).expect(
      "Build service channel closed while sending file change event - build service terminated unexpectedly"
    );
  }
}
