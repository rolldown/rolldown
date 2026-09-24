use std::{
  fs, io,
  path::{Path, PathBuf},
};

use notify::{
  Event as NotifyEvent, EventKind,
  event::{MetadataKind, ModifyKind, RenameMode},
};
use rolldown_common::WatcherChangeKind;
use walkdir::WalkDir;

use crate::FsEvent;

/// Translates a notify event into [`FsEvent`]s, the same way for build watch and bundled dev.
///
/// The backends report a kind and a path, and the kind is not always right: FSEvents reports
/// `Name(Any)` for both sides of a rename and repeats earlier flags of a path, and no backend
/// reports the files of a directory that appears. So the kind is taken where it is clear, and
/// the disk decides the rest:
///
/// - Only files are reported, like the `add`/`change`/`unlink` events of chokidar. A directory
///   that appears is reported as `Create` of every file below it; a directory that disappears is
///   reported as `Delete` of the directory, since its files are unknown by then; a modified
///   directory is not reported.
/// - A path that no longer exists is reported as `Delete`, whatever the backend said.
/// - A metadata change is reported as `Update` when it can be a write: `touch` is one on every
///   platform, and polling reports every write as `WriteTime`. Permission, ownership, extended
///   attribute and access time changes are not reported.
///
/// Renames map `Name(From)` → `Delete`, `Name(To)` → `Create` and `Name(Both)` → `Delete` for
/// `paths[0]`, `Create` for `paths[1]`. `Access` and unknown kinds produce nothing; reading
/// watched files on Linux would otherwise loop (`IN_OPEN`).
///
/// See `internal-docs/watch-mode/implementation.md` ("Notify Event Mapping").
pub fn map_notify_event(event: NotifyEvent, events: &mut Vec<FsEvent>) {
  match event.kind {
    EventKind::Create(_)
    | EventKind::Modify(ModifyKind::Name(RenameMode::To | RenameMode::Any | RenameMode::Other)) => {
      for path in event.paths {
        push_present(path, WatcherChangeKind::Create, events);
      }
    }
    EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
      events
        .extend(event.paths.into_iter().map(|path| FsEvent::new(path, WatcherChangeKind::Delete)));
    }
    EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
      // `[from_path, to_path]`, extra paths are ignored.
      let mut paths = event.paths.into_iter();
      if let Some(from) = paths.next() {
        events.push(FsEvent::new(from, WatcherChangeKind::Delete));
      }
      if let Some(to) = paths.next() {
        push_present(to, WatcherChangeKind::Create, events);
      }
    }
    EventKind::Modify(ModifyKind::Metadata(kind))
      if !matches!(kind, MetadataKind::Any | MetadataKind::WriteTime) => {}
    EventKind::Modify(_) => {
      for path in event.paths {
        push_present(path, WatcherChangeKind::Update, events);
      }
    }
    _ => {}
  }
}

/// Reports `kind` for a path that should exist now, after a look at the disk: a directory stands
/// for the files below it, a missing path is deleted.
fn push_present(path: PathBuf, kind: WatcherChangeKind, events: &mut Vec<FsEvent>) {
  match fs::symlink_metadata(&path) {
    Ok(metadata) if metadata.is_dir() => {
      if kind == WatcherChangeKind::Create {
        push_files_below(&path, events);
      }
    }
    Err(error) if error.kind() == io::ErrorKind::NotFound => {
      events.push(FsEvent::new(path, WatcherChangeKind::Delete));
    }
    _ => events.push(FsEvent::new(path, kind)),
  }
}

fn push_files_below(dir: &Path, events: &mut Vec<FsEvent>) {
  let files = WalkDir::new(dir)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| !entry.file_type().is_dir())
    .map(|entry| FsEvent::new(entry.into_path(), WatcherChangeKind::Create));
  events.extend(files);
}

#[cfg(all(test, not(windows)))]
mod tests {
  use std::{
    fs,
    path::{Path, PathBuf},
  };

  use notify::{
    Event as NotifyEvent, EventKind,
    event::{AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind, RenameMode},
  };
  use rolldown_common::WatcherChangeKind::{self, Create, Delete, Update};

  use super::map_notify_event;
  use crate::FsEvent;

  /// A fresh directory holding `a.js`, `nested/b.js` and the empty directory `nested/empty`.
  struct Fixture(PathBuf);

  impl Fixture {
    fn new(name: &str) -> Self {
      let dir =
        std::env::temp_dir().join(format!("rolldown_fs_watcher_{}_{name}", std::process::id()));
      let _ = fs::remove_dir_all(&dir);
      fs::create_dir_all(dir.join("nested/empty")).unwrap();
      fs::write(dir.join("a.js"), "").unwrap();
      fs::write(dir.join("nested/b.js"), "").unwrap();
      Self(dir)
    }

    fn path(&self, path: &str) -> PathBuf {
      self.0.join(path)
    }
  }

  impl Drop for Fixture {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }

  fn map(kind: EventKind, paths: &[&Path]) -> Vec<(PathBuf, WatcherChangeKind)> {
    let mut event = NotifyEvent::new(kind);
    event.paths = paths.iter().map(|path| path.to_path_buf()).collect();
    let mut events = Vec::new();
    map_notify_event(event, &mut events);
    events.sort_by(|a, b| a.path.cmp(&b.path));
    events.into_iter().map(|FsEvent { path, kind }| (path, kind)).collect()
  }

  #[test]
  fn file_events() {
    let fixture = Fixture::new("file_events");
    let a = fixture.path("a.js");
    assert_eq!(map(EventKind::Create(CreateKind::File), &[&a]), [(a.clone(), Create)]);
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Data(DataChange::Content)), &[&a]),
      [(a.clone(), Update)]
    );
    assert_eq!(map(EventKind::Remove(RemoveKind::File), &[&a]), [(a.clone(), Delete)]);
    assert!(map(EventKind::Access(AccessKind::Read), &[&a]).is_empty());
    assert!(map(EventKind::Any, &[&a]).is_empty());
  }

  #[test]
  fn rename_events() {
    let fixture = Fixture::new("rename_events");
    let (old, new) = (fixture.path("old.js"), fixture.path("a.js"));
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::From)), &[&old]),
      [(old.clone(), Delete)]
    );
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::To)), &[&new]),
      [(new.clone(), Create)]
    );
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &[&old, &new]),
      [(new.clone(), Create), (old.clone(), Delete)]
    );
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &[&old]),
      [(old.clone(), Delete)]
    );
    // FSEvents reports both sides of a rename as `Any`; the disk tells them apart.
    assert_eq!(map(EventKind::Modify(ModifyKind::Name(RenameMode::Any)), &[&old]), [(old, Delete)]);
    assert_eq!(map(EventKind::Modify(ModifyKind::Name(RenameMode::Any)), &[&new]), [(new, Create)]);
  }

  #[test]
  fn missing_path_events() {
    let fixture = Fixture::new("missing_path_events");
    let missing = fixture.path("missing.js");
    assert_eq!(map(EventKind::Create(CreateKind::File), &[&missing]), [(missing.clone(), Delete)]);
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Data(DataChange::Content)), &[&missing]),
      [(missing, Delete)]
    );
  }

  #[test]
  fn metadata_events() {
    let fixture = Fixture::new("metadata_events");
    let a = fixture.path("a.js");
    for kind in [MetadataKind::Any, MetadataKind::WriteTime] {
      assert_eq!(map(EventKind::Modify(ModifyKind::Metadata(kind)), &[&a]), [(a.clone(), Update)]);
    }
    for kind in [
      MetadataKind::AccessTime,
      MetadataKind::Permissions,
      MetadataKind::Ownership,
      MetadataKind::Extended,
      MetadataKind::Other,
    ] {
      assert!(map(EventKind::Modify(ModifyKind::Metadata(kind)), &[&a]).is_empty());
    }
  }

  #[test]
  fn directory_events() {
    let fixture = Fixture::new("directory_events");
    let dir = fixture.0.clone();

    // a directory that appears stands for the files below it
    let files_below = [(fixture.path("a.js"), Create), (fixture.path("nested/b.js"), Create)];
    for kind in [
      EventKind::Create(CreateKind::Folder),
      EventKind::Modify(ModifyKind::Name(RenameMode::To)),
      EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
    ] {
      assert_eq!(map(kind, &[&dir]), files_below);
    }
    assert!(
      map(EventKind::Create(CreateKind::Folder), &[&fixture.path("nested/empty")]).is_empty()
    );

    // a modified directory is not reported
    assert!(map(EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)), &[&dir]).is_empty());
    assert!(map(EventKind::Modify(ModifyKind::Data(DataChange::Content)), &[&dir]).is_empty());

    // a directory that disappears is reported itself
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(map(EventKind::Remove(RemoveKind::Folder), &[&dir]), [(dir.clone(), Delete)]);
    assert_eq!(
      map(EventKind::Modify(ModifyKind::Name(RenameMode::From)), &[&dir]),
      [(dir, Delete)]
    );
  }
}
