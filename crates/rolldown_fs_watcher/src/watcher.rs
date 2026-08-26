use std::path::Path;

use notify::RecursiveMode;
use rolldown_error::BuildResult;

use crate::{FsEventHandler, FsWatcherConfig};

/// The filesystem watcher used by Rolldown.
///
/// Construction selects the notify implementation from [`FsWatcherConfig`]. The concrete
/// recommended, polling, immediate, and debounced types remain internal.
pub struct FsWatcher {
  backend: Box<dyn WatcherBackend>,
}

impl FsWatcher {
  pub fn new<F: FsEventHandler>(event_handler: F, config: &FsWatcherConfig) -> BuildResult<Self> {
    Ok(Self { backend: crate::notify::create_backend(event_handler, config)? })
  }

  pub fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()> {
    self.backend.watch(path, recursive_mode)
  }

  /// Stop watching a path.
  pub fn unwatch(&mut self, path: &Path) -> BuildResult<()> {
    self.backend.unwatch(path)
  }

  /// Returns a mutable interface to the watched paths for batch operations.
  pub fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
    self.backend.paths_mut()
  }
}

pub trait WatcherBackend: Send {
  fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()>;

  fn unwatch(&mut self, path: &Path) -> BuildResult<()>;

  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_>;
}

/// A trait for batch manipulation of watched paths.
pub trait PathsMut {
  fn add(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()>;

  fn remove(&mut self, path: &Path) -> BuildResult<()>;

  fn commit(self: Box<Self>) -> BuildResult<()>;
}
