use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use rolldown_error::BuildResult;
use rustc_hash::FxHashSet;

use crate::{
  FsEventHandler, FsWatcherConfig,
  filter::IgnoreFilter,
  notify::{WatcherBackend, create_backend},
};

pub struct FsWatcher {
  backend: Box<dyn WatcherBackend>,
  /// Shared with the backend as notify's ignore filter.
  filter: Option<Arc<IgnoreFilter>>,
  watched_paths: FxHashSet<PathBuf>,
  add_restarts_stream: bool,
}

impl FsWatcher {
  pub fn new<F: FsEventHandler>(event_handler: F, config: &FsWatcherConfig) -> BuildResult<Self> {
    let filter = config.ignore_filter()?.map(Arc::new);
    Ok(Self {
      backend: create_backend(event_handler, config, filter.clone())?,
      filter,
      watched_paths: FxHashSet::default(),
      // The macOS native backend is FSEvents (notify's default `macos_fsevent` feature).
      add_restarts_stream: cfg!(target_os = "macos") && config.enabled && !config.use_polling,
    })
  }

  /// Paths must be absolute and normalized (`WatchPath` in `rolldown_common`). A directory is
  /// watched with everything below it. A path notify cannot watch is skipped and tried again
  /// next time.
  pub fn watch_paths<P: AsRef<Path>>(
    &mut self,
    paths: impl IntoIterator<Item = P>,
    mut is_wanted: impl FnMut(&Path) -> bool,
  ) -> BuildResult<()> {
    let mut new_paths = FxHashSet::default();
    for path in paths {
      let path = path.as_ref();
      if self.watched_paths.contains(path)
        || new_paths.contains(path)
        || !is_wanted(path)
        || self.filter.as_ref().is_some_and(|filter| filter.is_path_ignored(path))
      {
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
    self.watched_paths.extend(new_paths);
    Ok(())
  }

  pub fn is_watched(&self, path: &Path) -> bool {
    path.ancestors().any(|ancestor| self.watched_paths.contains(ancestor))
  }

  /// Exact match only, unlike `is_watched`.
  pub fn is_registered(&self, path: &Path) -> bool {
    self.watched_paths.contains(path)
  }

  /// True when committing a non-empty batch restarts the backend's event stream, so edits made
  /// meanwhile can be lost. That is the FSEvents backend (macOS native, not polling); it also
  /// covers everything below a watched directory in the kernel. False for the polling backend,
  /// the disabled backend, and every other platform.
  pub fn add_restarts_stream(&self) -> bool {
    self.add_restarts_stream
  }

  pub fn watched_paths(&self) -> impl Iterator<Item = &Path> {
    self.watched_paths.iter().map(PathBuf::as_path)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    sync::mpsc,
    time::{Duration, Instant},
  };

  use rolldown_utils::pattern_filter::StringOrRegex;

  use super::*;
  use crate::{FsEvent, notify::PathsMut};

  struct UnexpectedBatch;

  impl WatcherBackend for UnexpectedBatch {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      panic!("an unchanged watch set must not open a native watcher batch");
    }
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
      watched_paths: FxHashSet::default(),
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
      watched_paths: FxHashSet::from_iter([path.clone()]),
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
      watched_paths: FxHashSet::from_iter([watched.clone()]),
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
      watched_paths: FxHashSet::default(),
      add_restarts_stream: false,
    };
    let ignored = root.join("debug.log");
    watcher.watch_paths([&ignored], |_| true).unwrap();
    assert!(!watcher.is_watched(&ignored));
  }

  #[cfg(not(windows))]
  #[test]
  fn watch_paths() {
    struct NoopHandler;

    impl FsEventHandler for NoopHandler {
      fn handle_events(&mut self, _events: Vec<crate::FsEvent>) {}
    }

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
