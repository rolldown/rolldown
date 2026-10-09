use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use rolldown_error::BuildResult;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::{
  FsEventHandler, FsWatcherConfig,
  filter::IgnoreFilter,
  notify::{WatcherBackend, create_backend},
};

/// Identifies a caller of a watcher shared by several callers, such as the output tasks of one
/// watch-mode config. A watcher with one caller uses `watch_paths`, which records `SOLE_OWNER`.
pub type WatchOwner = usize;

const SOLE_OWNER: WatchOwner = 0;

pub struct FsWatcher {
  backend: Box<dyn WatcherBackend>,
  /// Shared with the backend as notify's ignore filter.
  filter: Option<Arc<IgnoreFilter>>,
  /// Every path a caller watches, with the callers that asked for it.
  watched_paths: FxHashMap<PathBuf, SmallVec<[WatchOwner; 2]>>,
  /// Committing a non-empty batch restarts the backend's event stream, so edits made meanwhile
  /// can be lost. That is the FSEvents backend (macOS native, not polling); it also covers
  /// everything below a watched directory in the kernel.
  add_restarts_stream: bool,
}

impl FsWatcher {
  pub fn new<F: FsEventHandler>(event_handler: F, config: &FsWatcherConfig) -> BuildResult<Self> {
    let filter = config.ignore_filter()?.map(Arc::new);
    Ok(Self {
      backend: create_backend(event_handler, config, filter.clone())?,
      filter,
      watched_paths: FxHashMap::default(),
      // The macOS native backend is FSEvents (notify's default `macos_fsevent` feature).
      add_restarts_stream: cfg!(target_os = "macos") && config.enabled && !config.use_polling,
    })
  }

  /// Paths must be absolute and normalized (`WatchPath` in `rolldown_common`). A directory is
  /// watched with everything below it. A path notify cannot watch is skipped and tried again
  /// next time. For a watcher with one caller: every new path is registered with the backend,
  /// even one below a watched directory.
  pub fn watch_paths<P: AsRef<Path>>(
    &mut self,
    paths: impl IntoIterator<Item = P>,
    is_wanted: impl FnMut(&Path) -> bool,
  ) -> BuildResult<()> {
    self.add_paths(SOLE_OWNER, paths, is_wanted, false)
  }

  /// `watch_paths` for one of several callers sharing this watcher. A path another caller
  /// already watches is recorded for `owner` without a backend batch. Where a batch restarts
  /// the event stream (FSEvents), so is a path below a watched directory: the kernel reports it
  /// through that directory. Elsewhere such a path is still registered, so it stays watched
  /// even when the backend could not extend the directory's recursive watch (inotify with a
  /// full watch table).
  /// See internal-docs/watch-mode/implementation.md ("File Watching").
  pub fn watch_paths_as<P: AsRef<Path>>(
    &mut self,
    owner: WatchOwner,
    paths: impl IntoIterator<Item = P>,
    is_wanted: impl FnMut(&Path) -> bool,
  ) -> BuildResult<()> {
    let skip_covered = self.add_restarts_stream;
    self.add_paths(owner, paths, is_wanted, skip_covered)
  }

  fn add_paths<P: AsRef<Path>>(
    &mut self,
    owner: WatchOwner,
    paths: impl IntoIterator<Item = P>,
    mut is_wanted: impl FnMut(&Path) -> bool,
    skip_covered: bool,
  ) -> BuildResult<()> {
    let mut new_paths = FxHashSet::default();
    for path in paths {
      let path = path.as_ref();
      if self.watched_paths.get(path).is_some_and(|owners| owners.contains(&owner))
        || new_paths.contains(path)
        || !is_wanted(path)
        || self.filter.as_ref().is_some_and(|filter| filter.is_path_ignored(path))
      {
        continue;
      }
      if self.watched_paths.contains_key(path) || (skip_covered && self.is_watched(path)) {
        self.watched_paths.entry(path.to_path_buf()).or_default().push(owner);
        continue;
      }
      new_paths.insert(path.to_path_buf());
    }
    // Even an empty notify batch restarts the FSEvents stream and loses edits made meanwhile.
    // See internal-docs/watch-mode/implementation.md.
    if new_paths.is_empty() {
      return Ok(());
    }

    let mut paths_mut = self.backend.paths_mut();
    new_paths.retain(|path| match paths_mut.add(path) {
      Ok(()) => {
        tracing::debug!(name = "notify watch", ?path);
        true
      }
      Err(error) => {
        tracing::debug!(name = "notify watch skipped", ?path, ?error);
        false
      }
    });
    paths_mut.commit()?;
    for path in new_paths {
      self.watched_paths.entry(path).or_default().push(owner);
    }
    Ok(())
  }

  /// Whether any caller watches `path`, itself or through a watched directory.
  pub fn is_watched(&self, path: &Path) -> bool {
    path.ancestors().any(|ancestor| self.watched_paths.contains_key(ancestor))
  }

  /// Whether `owner` watches `path`, itself or through a watched directory.
  pub fn is_watched_by(&self, owner: WatchOwner, path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
      self.watched_paths.get(ancestor).is_some_and(|owners| owners.contains(&owner))
    })
  }

  pub fn watched_paths(&self) -> impl Iterator<Item = &Path> {
    self.watched_paths.keys().map(PathBuf::as_path)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    sync::mpsc,
    time::{Duration, Instant},
  };

  use rolldown_error::ResultExt;
  use rolldown_utils::pattern_filter::StringOrRegex;

  use super::*;
  use crate::{FsEvent, notify::PathsMut};

  struct UnexpectedBatch;

  impl WatcherBackend for UnexpectedBatch {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      panic!("an unchanged watch set must not open a native watcher batch");
    }
  }

  struct NoopHandler;

  impl FsEventHandler for NoopHandler {
    fn handle_events(&mut self, _events: Vec<FsEvent>) {}
  }

  struct ChannelHandler(mpsc::Sender<Vec<FsEvent>>);

  impl FsEventHandler for ChannelHandler {
    fn handle_events(&mut self, events: Vec<FsEvent>) {
      self.0.send(events).unwrap();
    }
  }

  #[test]
  fn empty_watch_paths_does_not_open_batch() {
    let mut watcher = FsWatcher {
      backend: Box::new(UnexpectedBatch),
      filter: None,
      watched_paths: FxHashMap::default(),
      add_restarts_stream: false,
    };
    watcher.watch_paths(std::iter::empty::<&Path>(), |_| panic!("no paths to filter")).unwrap();
  }

  #[test]
  fn already_watched_paths_does_not_open_batch() {
    let path = std::env::current_dir().unwrap().join("entry.js");
    let mut watcher = FsWatcher {
      backend: Box::new(UnexpectedBatch),
      filter: None,
      watched_paths: FxHashMap::from_iter([(path.clone(), SmallVec::from_elem(SOLE_OWNER, 1))]),
      add_restarts_stream: false,
    };
    watcher
      .watch_paths([&path, &path], |_| panic!("already watched paths must not be filtered again"))
      .unwrap();
    assert!(watcher.is_watched(&path));
  }

  #[test]
  fn excluded_paths_does_not_open_batch() {
    let root = std::env::current_dir().unwrap();
    let watched = root.join("entry.js");
    let excluded = root.join("excluded.js");
    let mut watcher = FsWatcher {
      backend: Box::new(UnexpectedBatch),
      filter: None,
      watched_paths: FxHashMap::from_iter([(watched.clone(), SmallVec::from_elem(SOLE_OWNER, 1))]),
      add_restarts_stream: false,
    };
    let mut asked = Vec::new();
    watcher
      .watch_paths([&watched, &excluded, &watched], |path| {
        asked.push(path.to_path_buf());
        false
      })
      .unwrap();
    assert_eq!(asked, std::slice::from_ref(&excluded));
    assert!(!watcher.is_watched(&excluded));
  }

  #[test]
  fn ignored_paths_does_not_open_batch() {
    let root = std::env::current_dir().unwrap();
    let config = FsWatcherConfig {
      ignored: Some(vec![StringOrRegex::String("**/*.log".to_string())]),
      cwd: root.clone(),
      ..FsWatcherConfig::default()
    };
    let mut watcher = FsWatcher {
      backend: Box::new(UnexpectedBatch),
      filter: config.ignore_filter().unwrap().map(Arc::new),
      watched_paths: FxHashMap::default(),
      add_restarts_stream: false,
    };
    let ignored = root.join("debug.log");
    watcher.watch_paths([&ignored], |_| true).unwrap();
    assert!(!watcher.is_watched(&ignored));
  }

  #[cfg(not(windows))]
  #[test]
  fn watch_paths() {
    let config = FsWatcherConfig { enabled: false, ..FsWatcherConfig::default() };
    let mut watcher = FsWatcher::new(NoopHandler, &config).unwrap();

    let mut asked = Vec::new();
    watcher
      .watch_paths(
        ["/project/src/index.js", "/project/lib", "/project/src/index.js", "/project/skipped.js"],
        |path| {
          asked.push(path.to_path_buf());
          !path.ends_with("skipped.js")
        },
      )
      .unwrap();

    assert_eq!(
      asked,
      [
        PathBuf::from("/project/src/index.js"),
        PathBuf::from("/project/lib"),
        PathBuf::from("/project/skipped.js")
      ]
    );
    let mut watched: Vec<_> = watcher.watched_paths().collect();
    watched.sort();
    assert_eq!(watched, [Path::new("/project/lib"), Path::new("/project/src/index.js")]);

    assert!(watcher.is_watched(Path::new("/project/src/index.js")));
    assert!(watcher.is_watched(Path::new("/project/lib/nested/index.js")));
    assert!(!watcher.is_watched(Path::new("/project/lib-other/index.js")));
    assert!(!watcher.is_watched(Path::new("/project/skipped.js")));
  }

  /// Records every path the watcher registers with the backend.
  #[derive(Default)]
  struct RecordingBackend {
    added: Arc<std::sync::Mutex<Vec<PathBuf>>>,
    refused: Option<PathBuf>,
  }

  impl WatcherBackend for RecordingBackend {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      Box::new(RecordingPathsMut(self))
    }
  }

  struct RecordingPathsMut<'a>(&'a mut RecordingBackend);

  impl PathsMut for RecordingPathsMut<'_> {
    fn add(&mut self, path: &Path) -> BuildResult<()> {
      if self.0.refused.as_deref() == Some(path) {
        let refused: Result<(), std::io::Error> = Err(std::io::ErrorKind::Other.into());
        return refused.map_err_to_unhandleable().map_err(Into::into);
      }
      self.0.added.lock().unwrap().push(path.to_path_buf());
      Ok(())
    }

    fn commit(self: Box<Self>) -> BuildResult<()> {
      Ok(())
    }
  }

  fn recording_watcher(
    add_restarts_stream: bool,
    refused: Option<PathBuf>,
  ) -> (FsWatcher, Arc<std::sync::Mutex<Vec<PathBuf>>>) {
    let added = Arc::default();
    let backend = RecordingBackend { added: Arc::clone(&added), refused };
    let watcher = FsWatcher {
      backend: Box::new(backend),
      filter: None,
      watched_paths: FxHashMap::default(),
      add_restarts_stream,
    };
    (watcher, added)
  }

  /// Owner 0 watches `assets/`, then owner 1 asks for `assets/x.svg`. Returns the paths the
  /// backend registered, and checks that membership stays per owner.
  fn watch_file_below_other_owners_directory(add_restarts_stream: bool) -> Vec<PathBuf> {
    let root = std::env::current_dir().unwrap();
    let assets = root.join("assets");
    let svg = assets.join("x.svg");
    let (mut watcher, added) = recording_watcher(add_restarts_stream, None);

    watcher.watch_paths_as(0, [&assets], |_| true).unwrap();
    watcher.watch_paths_as(1, [&svg], |_| true).unwrap();

    assert!(watcher.is_watched_by(1, &svg), "owner 1 must own the covered file");
    assert!(watcher.is_watched_by(0, &svg), "owner 0 watches it through `assets/`");
    assert!(!watcher.is_watched_by(1, &assets.join("y.svg")), "owner 1 does not watch `assets/`");
    added.lock().unwrap().clone()
  }

  /// On FSEvents a batch restarts the stream, so a file below a directory another owner
  /// already watches is recorded without one: the kernel reports it through the directory.
  #[test]
  fn path_covered_by_other_owners_directory_is_recorded_without_batch() {
    let added = watch_file_below_other_owners_directory(true);
    assert_eq!(added, [std::env::current_dir().unwrap().join("assets")]);
  }

  /// Elsewhere a covered file is still registered: the redundant add is harmless, and an
  /// explicit watch survives a recursive watch the backend could not extend.
  #[test]
  fn path_covered_by_other_owners_directory_is_registered_on_other_backends() {
    let added = watch_file_below_other_owners_directory(false);
    let assets = std::env::current_dir().unwrap().join("assets");
    assert_eq!(added, [assets.clone(), assets.join("x.svg")]);
  }

  #[test]
  fn path_another_owner_registered_is_recorded_without_batch() {
    let path = std::env::current_dir().unwrap().join("entry.js");
    let (mut watcher, added) = recording_watcher(false, None);
    watcher.watch_paths_as(0, [&path], |_| true).unwrap();
    assert!(!watcher.is_watched_by(1, &path));
    watcher.watch_paths_as(1, [&path], |_| true).unwrap();
    assert!(watcher.is_watched_by(1, &path));
    assert_eq!(*added.lock().unwrap(), [path]);
  }

  #[test]
  fn refused_path_is_not_recorded_and_is_retried() {
    let path = std::env::current_dir().unwrap().join("entry.js");
    let (mut watcher, added) = recording_watcher(false, Some(path.clone()));
    watcher.watch_paths_as(0, [&path], |_| true).unwrap();
    assert!(!watcher.is_watched(&path));
    watcher.backend = Box::new(RecordingBackend { added: Arc::clone(&added), refused: None });
    watcher.watch_paths_as(0, [&path], |_| true).unwrap();
    assert!(watcher.is_watched_by(0, &path));
    assert_eq!(*added.lock().unwrap(), [path]);
  }

  /// Only the native macOS backend restarts its stream on a batch.
  #[test]
  fn add_restarts_stream_only_for_fsevents() {
    let native = FsWatcher::new(NoopHandler, &FsWatcherConfig::default()).unwrap();
    assert_eq!(native.add_restarts_stream, cfg!(target_os = "macos"));
    let polling = FsWatcherConfig { use_polling: true, ..FsWatcherConfig::default() };
    assert!(!FsWatcher::new(NoopHandler, &polling).unwrap().add_restarts_stream);
    let disabled = FsWatcherConfig { enabled: false, ..FsWatcherConfig::default() };
    assert!(!FsWatcher::new(NoopHandler, &disabled).unwrap().add_restarts_stream);
  }

  /// The poll backend behaves the same on every platform.
  #[test]
  fn excluded_paths_are_not_reported() {
    let root =
      std::env::temp_dir().join(format!("rolldown_fs_watcher_excluded_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let seed = root.join("seed.js");
    fs::write(&seed, "0").unwrap();
    let (tx, rx) = mpsc::channel();
    let config = FsWatcherConfig {
      use_polling: true,
      poll_interval: 10,
      ignored: Some(vec![StringOrRegex::String("**/*.log".to_string())]),
      cwd: root.clone(),
      ..FsWatcherConfig::default()
    };
    let mut watcher = FsWatcher::new(ChannelHandler(tx), &config).unwrap();
    watcher.watch_paths([&root], |_| true).unwrap();

    // The poll backend takes its baseline on the scan after `watch_paths` returns, and a file
    // created before that scan is never reported. Touch a known file until the backend reports
    // it, which proves the baseline is taken.
    let mut paths = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    for tick in 1.. {
      fs::write(&seed, tick.to_string()).unwrap();
      match rx.recv_timeout(Duration::from_millis(50)) {
        Ok(events) if events.iter().any(|event| event.path == seed) => break,
        Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {
          assert!(Instant::now() < deadline, "the poll backend never reported seed.js");
        }
        Err(error) => panic!("{error}"),
      }
    }

    fs::write(root.join("debug.log"), "").unwrap();
    fs::write(root.join("index.js"), "").unwrap();

    let index = root.join("index.js");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !paths.contains(&index) {
      let events = rx
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .expect("index.js must be reported");
      paths.extend(events.into_iter().map(|event| event.path));
    }
    // give a wrongly reported debug.log time to arrive
    while let Ok(events) = rx.recv_timeout(Duration::from_millis(200)) {
      paths.extend(events.into_iter().map(|event| event.path));
    }
    assert!(!paths.contains(&root.join("debug.log")), "{paths:?}");
    drop(watcher);
    let _ = fs::remove_dir_all(&root);
  }

  #[cfg(target_os = "macos")]
  mod native {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
      fn new(use_debounce: bool) -> Self {
        let path = std::env::temp_dir()
          .join(format!("rolldown_empty_watch_batch_{}_{use_debounce}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        // FSEvents reports canonical paths, including macOS's /var -> /private/var alias.
        Self(path.canonicalize().unwrap())
      }
    }

    impl Drop for Fixture {
      fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
      }
    }

    fn assert_events_survive_filtered_batch(use_debounce: bool) {
      let fixture = Fixture::new(use_debounce);
      let entry = fixture.0.join("entry.js");
      let excluded = fixture.0.join("excluded.js");
      fs::write(&entry, "export const value = 0").unwrap();
      let (tx, rx) = mpsc::channel();
      let config = FsWatcherConfig { use_debounce, ..FsWatcherConfig::default() };
      let mut watcher = FsWatcher::new(ChannelHandler(tx), &config).unwrap();
      watcher.watch_paths([&entry], |_| true).unwrap();

      watcher
        .watch_paths([&excluded], |_| {
          // Place a save inside the old stopped-stream window
          // without relying on sleeps or a burst race.
          fs::write(&entry, "export const value = 1").unwrap();
          let events = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("filtering an unchanged watch set must not suspend native event delivery");
          assert!(events.iter().any(|event| event.path == entry));
          false
        })
        .unwrap();
      assert!(!watcher.is_watched(&excluded));
    }

    #[test]
    fn immediate_events_survive_filtered_batch() {
      assert_events_survive_filtered_batch(false);
    }

    #[test]
    fn debounced_events_survive_filtered_batch() {
      assert_events_survive_filtered_batch(true);
    }
  }
}
