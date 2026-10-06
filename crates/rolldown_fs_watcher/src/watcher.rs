use std::path::{Path, PathBuf};

use rolldown_error::BuildResult;
use rustc_hash::FxHashSet;

use crate::{
  FsEventHandler, FsWatcherConfig,
  notify::{WatcherBackend, create_backend},
};

pub struct FsWatcher {
  backend: Box<dyn WatcherBackend>,
  watched_paths: FxHashSet<PathBuf>,
}

impl FsWatcher {
  pub fn new<F: FsEventHandler>(event_handler: F, config: &FsWatcherConfig) -> BuildResult<Self> {
    Ok(Self {
      backend: create_backend(event_handler, config)?,
      watched_paths: FxHashSet::default(),
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
      if self.watched_paths.contains(path) || new_paths.contains(path) || !is_wanted(path) {
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

  pub fn watched_paths(&self) -> impl Iterator<Item = &Path> {
    self.watched_paths.iter().map(PathBuf::as_path)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::notify::PathsMut;

  struct UnexpectedBatch;

  impl WatcherBackend for UnexpectedBatch {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      panic!("an unchanged watch set must not open a native watcher batch");
    }
  }

  #[test]
  fn empty_watch_paths_does_not_open_batch() {
    let mut watcher =
      FsWatcher { backend: Box::new(UnexpectedBatch), watched_paths: FxHashSet::default() };
    watcher.watch_paths(std::iter::empty::<&Path>(), |_| panic!("no paths to filter")).unwrap();
  }

  #[test]
  fn already_watched_paths_does_not_open_batch() {
    let path = std::env::current_dir().unwrap().join("entry.js");
    let mut watcher = FsWatcher {
      backend: Box::new(UnexpectedBatch),
      watched_paths: FxHashSet::from_iter([path.clone()]),
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
      watched_paths: FxHashSet::from_iter([watched.clone()]),
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

  #[cfg(target_os = "macos")]
  mod native {
    use std::{fs, sync::mpsc, time::Duration};

    use super::*;
    use crate::FsEvent;

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

    struct ChannelHandler(mpsc::Sender<Vec<FsEvent>>);

    impl FsEventHandler for ChannelHandler {
      fn handle_events(&mut self, events: Vec<FsEvent>) {
        self.0.send(events).unwrap();
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
