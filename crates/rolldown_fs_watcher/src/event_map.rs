use notify::{
  Event as NotifyEvent, EventKind,
  event::{ModifyKind, RenameMode},
};
use rolldown_common::WatcherChangeKind;

use crate::{FsEvent, FsWatcherConfig};

/// Translates notify events into [`FsEvent`]s, the same way for build watch and bundled dev.
///
/// Rename events map `Name(From)` → `Delete`, `Name(To)` → `Create` and `Name(Both)` → `Delete`
/// for `paths[0]`, `Create` for `paths[1]`. `Access` and unknown kinds produce nothing; reading
/// watched files on Linux would otherwise loop (`IN_OPEN`).
///
/// See `internal-docs/watch-mode/implementation.md` ("Notify Event Mapping").
pub struct EventMapper {
  /// Whether `Modify(Metadata(_))` is ignored: on macOS the native backend reports metadata
  /// changes often, and they do not affect a build in most cases. Polling reports a content
  /// change as `Metadata(WriteTime)`, so they must be kept there.
  ignore_metadata_events: bool,
}

impl EventMapper {
  pub fn new(config: &FsWatcherConfig) -> Self {
    Self { ignore_metadata_events: cfg!(target_os = "macos") && !config.use_polling }
  }

  pub fn map(&self, event: NotifyEvent, events: &mut Vec<FsEvent>) {
    match event.kind {
      EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
        events.extend(
          event.paths.into_iter().map(|path| FsEvent::new(path, WatcherChangeKind::Create)),
        );
      }
      EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
        events.extend(
          event.paths.into_iter().map(|path| FsEvent::new(path, WatcherChangeKind::Delete)),
        );
      }
      EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
        // `[from_path, to_path]`, extra paths are ignored.
        let mut paths = event.paths.into_iter();
        if let Some(from) = paths.next() {
          events.push(FsEvent::new(from, WatcherChangeKind::Delete));
        }
        if let Some(to) = paths.next() {
          events.push(FsEvent::new(to, WatcherChangeKind::Create));
        }
      }
      EventKind::Modify(ModifyKind::Metadata(_)) if self.ignore_metadata_events => {}
      EventKind::Modify(_) => {
        events.extend(
          event.paths.into_iter().map(|path| FsEvent::new(path, WatcherChangeKind::Update)),
        );
      }
      _ => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use notify::{
    Event as NotifyEvent, EventKind,
    event::{AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind, RenameMode},
  };
  use rolldown_common::WatcherChangeKind::{self, Create, Delete, Update};

  use super::EventMapper;
  use crate::FsEvent;

  fn map(kind: EventKind, paths: &[&str]) -> Vec<(PathBuf, WatcherChangeKind)> {
    map_with(&EventMapper { ignore_metadata_events: false }, kind, paths)
  }

  fn map_with(
    mapper: &EventMapper,
    kind: EventKind,
    paths: &[&str],
  ) -> Vec<(PathBuf, WatcherChangeKind)> {
    let mut event = NotifyEvent::new(kind);
    event.paths = paths.iter().map(PathBuf::from).collect();
    let mut events = Vec::new();
    mapper.map(event, &mut events);
    events.into_iter().map(|FsEvent { path, kind }| (path, kind)).collect()
  }

  fn path(s: &str) -> PathBuf {
    PathBuf::from(s)
  }

  #[test]
  fn file_events() {
    assert_eq!(map(EventKind::Create(CreateKind::File), &["a.js"]), [(path("a.js"), Create)]);
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Data(DataChange::Content)), &["a.js"]),
      [(path("a.js"), Update)]
    );
    assert_eq!(map(EventKind::Remove(RemoveKind::File), &["a.js"]), [(path("a.js"), Delete)]);
  }

  #[test]
  fn rename_events() {
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::From)), &["old.js"]),
      [(path("old.js"), Delete)]
    );
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::To)), &["new.js"]),
      [(path("new.js"), Create)]
    );
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &["old.js", "new.js"]),
      [(path("old.js"), Delete), (path("new.js"), Create)]
    );
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &["old.js"]),
      [(path("old.js"), Delete)]
    );
  }

  #[test]
  fn ignored_events() {
    assert!(map(EventKind::Access(AccessKind::Read), &["a.js"]).is_empty());
    assert!(map(EventKind::Any, &["a.js"]).is_empty());
  }

  #[test]
  fn metadata_events() {
    let kind = EventKind::Modify(ModifyKind::Metadata(MetadataKind::WriteTime));
    assert_eq!(map(kind, &["a.js"]), [(path("a.js"), Update)]);
    assert!(map_with(&EventMapper { ignore_metadata_events: true }, kind, &["a.js"]).is_empty());
  }
}
