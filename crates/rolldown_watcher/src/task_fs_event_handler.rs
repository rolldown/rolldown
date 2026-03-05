use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchTaskIdx;
use crate::watcher_msg::WatcherMsg;
use rolldown_common::WatcherChangeKind;
use rolldown_fs_watcher::{FsEventHandler, FsEventResult};
use tokio::sync::mpsc;

/// Bridge that maps raw notify events to `FileChangeEvent`s and forwards them
/// to the coordinator via the shared mpsc channel.
pub struct TaskFsEventHandler {
  pub task_index: WatchTaskIdx,
  pub tx: mpsc::UnboundedSender<WatcherMsg>,
}

impl TaskFsEventHandler {
  /// Map a notify `EventKind` to a `WatcherChangeKind`.
  ///
  /// Returns `None` for event kinds that should not trigger a rebuild.
  /// In particular, `Access` events (file open/read/close) are ignored because
  /// the build process itself reads watched source files, which would otherwise
  /// cause an infinite rebuild loop on Linux where inotify emits `IN_OPEN` events.
  fn map_event_kind(kind: &notify::EventKind) -> Option<WatcherChangeKind> {
    match kind {
      notify::EventKind::Create(_) => Some(WatcherChangeKind::Create),
      notify::EventKind::Remove(_) => Some(WatcherChangeKind::Delete),
      notify::EventKind::Modify(_) => Some(WatcherChangeKind::Update),
      _ => None,
    }
  }
}

impl FsEventHandler for TaskFsEventHandler {
  fn handle_event(&mut self, event: FsEventResult) {
    match event {
      Ok(fs_events) => {
        let changes: Vec<FileChangeEvent> = fs_events
          .into_iter()
          .filter_map(|fs_event| {
            let kind = Self::map_event_kind(&fs_event.detail.kind)?;
            Some(
              fs_event
                .detail
                .paths
                .into_iter()
                .map(move |path| FileChangeEvent::new(path.to_string_lossy().into_owned(), kind)),
            )
          })
          .flatten()
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
