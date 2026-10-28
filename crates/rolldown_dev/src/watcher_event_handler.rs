use rolldown_fs_watcher::{FsEvent, FsEventHandler};

use crate::{type_aliases::CoordinatorSender, types::coordinator_msg::CoordinatorMsg};

pub struct WatcherEventHandler {
  pub coordinator_tx: CoordinatorSender,
}
impl FsEventHandler for WatcherEventHandler {
  fn handle_events(&mut self, events: Vec<FsEvent>) {
    if self.coordinator_tx.send(CoordinatorMsg::WatchEvent(events)).is_err() {
      tracing::debug!(
        "[WatcherEventHandler] coordinator channel closed while sending file change event"
      );
    }
  }
}
