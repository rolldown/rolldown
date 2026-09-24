use std::path::{Path, PathBuf};

use notify::RecursiveMode;
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
    let mut paths_mut = self.backend.paths_mut();
    let mut added_paths = FxHashSet::default();
    for path in paths {
      let path = path.as_ref().to_path_buf();
      if self.watched_paths.contains(&path) || added_paths.contains(&path) || !is_wanted(&path) {
        continue;
      }
      match paths_mut.add(&path, RecursiveMode::Recursive) {
        Ok(()) => {
          tracing::debug!(name = "notify watch", ?path);
          added_paths.insert(path);
        }
        Err(error) => {
          tracing::debug!(name = "notify watch skipped", ?path, ?error);
        }
      }
    }
    paths_mut.commit()?;
    self.watched_paths.extend(added_paths);
    Ok(())
  }

  pub fn is_watched(&self, path: &Path) -> bool {
    path.ancestors().any(|ancestor| self.watched_paths.contains(ancestor))
  }

  pub fn watched_paths(&self) -> impl Iterator<Item = &Path> {
    self.watched_paths.iter().map(PathBuf::as_path)
  }
}

#[cfg(all(test, not(windows)))]
mod tests {
  use super::*;
  use crate::FsEventResult;

  struct NoopHandler;

  impl FsEventHandler for NoopHandler {
    fn handle_event(&mut self, _event: FsEventResult) {}
  }

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
}
