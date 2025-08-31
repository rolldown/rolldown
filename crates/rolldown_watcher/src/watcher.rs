use notify::RecursiveMode;
use rolldown_error::BuildResult;
use std::path::Path;

use crate::{EventHandler, WatcherConfig};

pub trait Watcher {
  fn new<F: EventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized;

  fn with_config<F: EventHandler>(event_handler: F, config: WatcherConfig) -> BuildResult<Self>
  where
    Self: Sized;

  fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()>;

  /// Stop watching a path.
  ///
  /// # Errors
  ///
  /// Returns an error in the case that `path` has not been watched or if removing the watch
  /// fails.
  fn unwatch(&mut self, path: &Path) -> BuildResult<()>;
}
