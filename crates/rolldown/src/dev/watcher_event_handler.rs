use rolldown_watcher::EventHandler;

use crate::dev::watcher_event_service::{WatcherEventServiceMsg, WatcherEventServiceTx};

pub struct WatcherEventHandler {
  pub service_tx: WatcherEventServiceTx,
}
impl EventHandler for WatcherEventHandler {
  fn handle_event(&mut self, event: rolldown_watcher::FileChangeResult) {
    if self.service_tx.send(WatcherEventServiceMsg::FileChange(event)).is_err() {
      // TODO: handle send failed
    }
  }
}
