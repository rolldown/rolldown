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
  ///
  /// Aligned with `BundleCoordinator::handle_watch_event` in `rolldown_dev`.
  fn map_event_kind(kind: &notify::EventKind) -> Option<WatcherChangeKind> {
    match kind {
      notify::EventKind::Create(_)
      | notify::EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::To)) => {
        Some(WatcherChangeKind::Create)
      }
      notify::EventKind::Modify(notify::event::ModifyKind::Name(
        notify::event::RenameMode::From,
      ))
      | notify::EventKind::Remove(_) => Some(WatcherChangeKind::Delete),
      notify::EventKind::Modify(_) => Some(WatcherChangeKind::Update),
      _ => None,
    }
  }

  /// Check if this event is a `RenameMode::Both` event, which carries two paths
  /// (source and destination) that need different change kinds.
  fn is_rename_both(kind: &notify::EventKind) -> bool {
    matches!(
      kind,
      notify::EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::Both))
    )
  }
}

impl FsEventHandler for TaskFsEventHandler {
  fn handle_event(&mut self, event: FsEventResult) {
    match event {
      Ok(fs_events) => {
        let changes: Vec<FileChangeEvent> = fs_events
          .into_iter()
          .filter_map(|fs_event| {
            // RenameMode::Both carries [from_path, to_path] — emit Delete for the
            // source and Create for the destination so both signals are preserved.
            if Self::is_rename_both(&fs_event.detail.kind) {
              let mut paths = fs_event.detail.paths.into_iter();
              let mut result = Vec::new();
              if let Some(from) = paths.next() {
                result.push(FileChangeEvent::new(
                  from.to_string_lossy().into_owned(),
                  WatcherChangeKind::Delete,
                ));
              }
              if let Some(to) = paths.next() {
                result.push(FileChangeEvent::new(
                  to.to_string_lossy().into_owned(),
                  WatcherChangeKind::Create,
                ));
              }
              return Some(result);
            }

            let kind = Self::map_event_kind(&fs_event.detail.kind)?;
            Some(
              fs_event
                .detail
                .paths
                .into_iter()
                .map(|path| FileChangeEvent::new(path.to_string_lossy().into_owned(), kind))
                .collect(),
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
