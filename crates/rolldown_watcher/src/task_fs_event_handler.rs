use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchTaskIdx;
use crate::watcher_msg::WatcherMsg;
use futures::channel::mpsc;
use rolldown_fs_watcher::{FsEvent, FsEventHandler};

/// Bridge that forwards file changes as `FileChangeEvent`s to the coordinator
/// via the shared mpsc channel.
pub struct TaskFsEventHandler {
  pub task_index: WatchTaskIdx,
  pub tx: mpsc::UnboundedSender<WatcherMsg>,
}

impl FsEventHandler for TaskFsEventHandler {
  fn handle_events(&mut self, events: Vec<FsEvent>) {
    let changes = events
      .into_iter()
      .map(|event| FileChangeEvent::new(event.path.to_string_lossy().into_owned(), event.kind))
      .collect();
    let _ =
      self.tx.unbounded_send(WatcherMsg::FileChanges { task_index: self.task_index, changes });
  }
}
