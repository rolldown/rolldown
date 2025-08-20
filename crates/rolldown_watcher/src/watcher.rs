use notify::RecursiveMode;
use notify_debouncer_full::DebounceEventHandler;
use rolldown_error::BuildResult;
use std::path::Path;

pub trait Watcher {
  fn new<F: DebounceEventHandler>(event_handler: F) -> BuildResult<Self>
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
