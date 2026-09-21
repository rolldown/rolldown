use std::path::{Path, PathBuf};

use notify::{
  EventKind,
  event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
};

/// Rolldown-level change kind produced from a notify event.
///
/// This is intentionally a local type so `rolldown_fs_watcher` does not depend
/// on `rolldown_common`. Callers map it onto `WatcherChangeKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsChangeKind {
  Create,
  Update,
  Delete,
}

/// Map a notify event to `(path, kind)` pairs.
///
/// Rename events are mapped the same way in build watch and bundled dev:
///
/// - `Name(From)` → `Delete` (move out of a watched path)
/// - `Name(To)` → `Create` (move onto a watched path)
/// - `Name(Both)` → `Delete` for `paths[0]`, `Create` for `paths[1]` (rename in place)
///
/// `Access` and other unhandled kinds produce no changes. `Access` is ignored
/// because reading watched files on Linux would otherwise loop (`IN_OPEN`).
///
/// See `internal-docs/watch-mode/implementation.md` ("Notify Event Mapping")
/// and `internal-docs/dev-engine/implementation.md` ("From fs event to queued task").
pub fn map_notify_event(kind: &EventKind, paths: Vec<PathBuf>) -> Vec<(PathBuf, FsChangeKind)> {
  fn is_file_candidate(path: &Path) -> bool {
    !path.metadata().is_ok_and(|metadata| metadata.is_dir())
  }

  match kind {
    EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder) => Vec::new(),
    EventKind::Create(CreateKind::File) => {
      paths.into_iter().map(|path| (path, FsChangeKind::Create)).collect()
    }
    EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
      paths.into_iter().map(|path| (path, FsChangeKind::Delete)).collect()
    }
    EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => paths
      .into_iter()
      .filter(|path| is_file_candidate(path))
      .map(|path| (path, FsChangeKind::Create))
      .collect(),
    EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
      if paths.get(1).is_some_and(|path| !is_file_candidate(path)) {
        return Vec::new();
      }
      let mut paths = paths.into_iter();
      let mut result = Vec::new();
      if let Some(from) = paths.next() {
        result.push((from, FsChangeKind::Delete));
      }
      if let Some(to) = paths.next() {
        result.push((to, FsChangeKind::Create));
      }
      result
    }
    EventKind::Any => paths
      .into_iter()
      .filter(|path| is_file_candidate(path))
      .map(|path| (path, FsChangeKind::Update))
      .collect(),
    EventKind::Modify(_) => paths
      .into_iter()
      .filter(|path| is_file_candidate(path))
      .map(|path| (path, FsChangeKind::Update))
      .collect(),
    _ => Vec::new(),
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use notify::{
    EventKind,
    event::{AccessKind, CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode},
  };

  use super::{FsChangeKind, map_notify_event};

  fn path(s: &str) -> PathBuf {
    PathBuf::from(s)
  }

  #[test]
  fn create_maps_to_create() {
    let got = map_notify_event(&EventKind::Create(CreateKind::File), vec![path("a.js")]);
    assert_eq!(got, vec![(path("a.js"), FsChangeKind::Create)]);
  }

  #[test]
  fn remove_maps_to_delete() {
    let got = map_notify_event(&EventKind::Remove(RemoveKind::File), vec![path("a.js")]);
    assert_eq!(got, vec![(path("a.js"), FsChangeKind::Delete)]);
  }

  #[test]
  fn content_modify_maps_to_update() {
    let got = map_notify_event(
      &EventKind::Modify(ModifyKind::Data(DataChange::Content)),
      vec![path("a.js")],
    );
    assert_eq!(got, vec![(path("a.js"), FsChangeKind::Update)]);
  }

  #[test]
  fn rename_from_maps_to_delete() {
    let got = map_notify_event(
      &EventKind::Modify(ModifyKind::Name(RenameMode::From)),
      vec![path("old.js")],
    );
    assert_eq!(got, vec![(path("old.js"), FsChangeKind::Delete)]);
  }

  #[test]
  fn rename_to_maps_to_create() {
    let got =
      map_notify_event(&EventKind::Modify(ModifyKind::Name(RenameMode::To)), vec![path("new.js")]);
    assert_eq!(got, vec![(path("new.js"), FsChangeKind::Create)]);
  }

  #[test]
  fn rename_both_maps_to_delete_then_create() {
    let got = map_notify_event(
      &EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
      vec![path("old.js"), path("new.js")],
    );
    assert_eq!(
      got,
      vec![(path("old.js"), FsChangeKind::Delete), (path("new.js"), FsChangeKind::Create)]
    );
  }

  #[test]
  fn rename_both_with_only_from_path_maps_to_delete() {
    let got = map_notify_event(
      &EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
      vec![path("old.js")],
    );
    assert_eq!(got, vec![(path("old.js"), FsChangeKind::Delete)]);
  }

  #[test]
  fn access_is_ignored() {
    let got = map_notify_event(&EventKind::Access(AccessKind::Read), vec![path("a.js")]);
    assert!(got.is_empty());
  }

  #[test]
  fn explicit_directory_event_is_ignored_by_file_mapper() {
    let got = map_notify_event(
      &EventKind::Create(CreateKind::Folder),
      vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))],
    );
    assert!(got.is_empty());
  }

  #[test]
  fn missing_path_is_a_file_candidate() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("missing-watch-candidate");
    let got = map_notify_event(&EventKind::Remove(RemoveKind::Any), vec![path.clone()]);
    assert_eq!(got, vec![(path, FsChangeKind::Delete)]);
  }
}
