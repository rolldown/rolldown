use rolldown_fs_watcher::FsEventHandler;

use crate::{type_aliases::CoordinatorSender, types::coordinator_msg::CoordinatorMsg};

pub struct WatcherEventHandler {
  pub coordinator_tx: CoordinatorSender,
}
impl FsEventHandler for WatcherEventHandler {
  fn handle_event(&mut self, event: rolldown_fs_watcher::FsEventResult) {
    self.coordinator_tx.send(CoordinatorMsg::WatchEvent(event)).expect(
      "Coordinator channel closed while sending file change event - coordinator terminated unexpectedly"
    );
  }
}
