use std::path::Path;

use notify::RecursiveMode;
use rolldown_error::BuildResult;

use crate::{EventHandler, Watcher, WatcherConfig};

/// A no-op watcher that does nothing. Used when file watching is disabled.
pub struct NoopWatcher;

impl Watcher for NoopWatcher {
  fn new<F: EventHandler>(_event_handler: F) -> BuildResult<Self> {
    Ok(Self)
  }

  fn with_config<F: EventHandler>(_event_handler: F, _config: WatcherConfig) -> BuildResult<Self> {
    Ok(Self)
  }

  fn watch(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
    Ok(())
  }

  fn unwatch(&mut self, _path: &Path) -> BuildResult<()> {
    Ok(())
  }
}
