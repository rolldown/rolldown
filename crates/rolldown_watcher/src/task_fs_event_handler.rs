use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchTaskIdx;
use crate::watcher_msg::WatcherMsg;
use rolldown_fs_watcher::{FsEventHandler, FsEventResult};
use tokio::sync::mpsc;

/// Bridge that forwards file changes as `FileChangeEvent`s to the coordinator
/// via the shared mpsc channel.
pub struct TaskFsEventHandler {
  pub task_index: WatchTaskIdx,
  pub tx: mpsc::UnboundedSender<WatcherMsg>,
}

impl FsEventHandler for TaskFsEventHandler {
  fn handle_event(&mut self, event: FsEventResult) {
    match event {
      Ok(fs_events) => {
        let changes = fs_events
          .into_iter()
          .map(|fs_event| {
            FileChangeEvent::new(fs_event.path.to_string_lossy().into_owned(), fs_event.kind)
          })
          .collect();
        let _ = self.tx.send(WatcherMsg::FileChanges { task_index: self.task_index, changes });
      }
      Err(errors) => {
        for e in errors {
          tracing::error!("notify error: {e:?}");
        }
      }
    }
  }
}
