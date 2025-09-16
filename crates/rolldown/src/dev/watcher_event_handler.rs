use rolldown_watcher::EventHandler;

use crate::dev::build_driver_service::{BuildDriverServiceTx, BuildMessage};

pub struct WatcherEventHandler {
  pub service_tx: BuildDriverServiceTx,
}
impl EventHandler for WatcherEventHandler {
  fn handle_event(&mut self, event: rolldown_watcher::FileChangeResult) {
    if self.service_tx.send(BuildMessage::FileChange(event)).is_err() {
      // TODO: handle send failed
    }
  }
}
