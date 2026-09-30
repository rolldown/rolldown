use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchGroupIdx;
use crate::watcher_msg::WatcherMsg;
use futures::channel::mpsc;
use rolldown_fs_watcher::{FsEvent, FsEventHandler};

/// Bridge that forwards file changes as `FileChangeEvent`s to the coordinator
/// via the shared mpsc channel. One handler exists per config group: every
/// output task of a config shares the same fs watcher, so one save produces
/// exactly one message carrying the group's identity.
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
    let _ =
      self.tx.unbounded_send(WatcherMsg::FileChanges { group_index: self.group_index, changes });
  }
}
