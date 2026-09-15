use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchTaskIdx;
use crate::watcher_msg::WatcherMsg;
use rolldown_common::WatcherChangeKind;
use rolldown_fs_watcher::{FsChangeKind, FsEventHandler, FsEventResult, map_notify_event};
use tokio::sync::mpsc;

/// Bridge that maps raw notify events to `FileChangeEvent`s and forwards them
/// to the coordinator via the shared mpsc channel.
pub struct TaskFsEventHandler {
  pub task_index: WatchTaskIdx,
  pub tx: mpsc::UnboundedSender<WatcherMsg>,
}

fn watcher_change_kind(kind: FsChangeKind) -> WatcherChangeKind {
  match kind {
    FsChangeKind::Create => WatcherChangeKind::Create,
    FsChangeKind::Update => WatcherChangeKind::Update,
    FsChangeKind::Delete => WatcherChangeKind::Delete,
  }
}

impl FsEventHandler for TaskFsEventHandler {
  fn handle_event(&mut self, event: FsEventResult) {
    match event {
      Ok(fs_events) => {
        // Shared with bundled dev via `map_notify_event`.
        // See `internal-docs/watch-mode/implementation.md` ("Notify Event Mapping").
        let changes: Vec<FileChangeEvent> = fs_events
          .into_iter()
          .flat_map(|fs_event| {
            map_notify_event(&fs_event.detail.kind, fs_event.detail.paths).into_iter().map(
              |(path, kind)| {
                FileChangeEvent::new(path.to_string_lossy().into_owned(), watcher_change_kind(kind))
              },
            )
          })
          .collect();

        if !changes.is_empty() {
          let _ = self.tx.send(WatcherMsg::FileChanges { task_index: self.task_index, changes });
        }
      }
      Err(errors) => {
        for e in errors {
          tracing::error!("notify error: {e:?}");
        }
      }
    }
  }
}
