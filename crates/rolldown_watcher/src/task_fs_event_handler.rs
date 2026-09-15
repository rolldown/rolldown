use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchGroupIdx;
use crate::watcher_msg::WatcherMsg;
use futures::channel::mpsc;
use rolldown_common::WatcherChangeKind;
use rolldown_fs_watcher::{FsChangeKind, FsEventHandler, FsEventResult, map_notify_event};

/// Bridge that maps raw notify events to `FileChangeEvent`s and forwards them
/// to the coordinator via the shared mpsc channel. One handler exists per
/// config group: every output task of a config shares the same fs watcher, so
/// one save produces exactly one message carrying the group's identity.
pub struct GroupFsEventHandler {
  pub group_index: WatchGroupIdx,
  pub tx: mpsc::UnboundedSender<WatcherMsg>,
}

fn watcher_change_kind(kind: FsChangeKind) -> WatcherChangeKind {
  match kind {
    FsChangeKind::Create => WatcherChangeKind::Create,
    FsChangeKind::Update => WatcherChangeKind::Update,
    FsChangeKind::Delete => WatcherChangeKind::Delete,
  }
}

impl FsEventHandler for GroupFsEventHandler {
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
          let _ = self
            .tx
            .unbounded_send(WatcherMsg::FileChanges { group_index: self.group_index, changes });
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
