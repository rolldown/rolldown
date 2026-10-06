use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchGroupIdx;
use crate::watcher_msg::WatcherMsg;
use rolldown_fs_watcher::{FsEvent, FsEventHandler};
use tokio::sync::mpsc;

/// Bridge that forwards file changes as `FileChangeEvent`s to the coordinator
/// via the shared mpsc channel. One per config group, so a save sends one message.
pub struct GroupFsEventHandler {
  pub group_index: WatchGroupIdx,
  pub tx: mpsc::UnboundedSender<WatcherMsg>,
}

impl FsEventHandler for GroupFsEventHandler {
  fn handle_events(&mut self, events: Vec<FsEvent>) {
    let changes = events
      .into_iter()
      .map(|event| FileChangeEvent::new(event.path.to_string_lossy().into_owned(), event.kind))
      .collect();
    let _ = self.tx.send(WatcherMsg::FileChanges { group_index: self.group_index, changes });
  }
}
