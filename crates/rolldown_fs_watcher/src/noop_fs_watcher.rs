use std::path::Path;

use notify::RecursiveMode;
use rolldown_error::BuildResult;

use crate::{FsEventHandler, FsWatcher, FsWatcherConfig};

/// A no-op filesystem watcher that does nothing. Used when file watching is disabled.
pub struct NoopFsWatcher;

impl FsWatcher for NoopFsWatcher {
  fn new<F: FsEventHandler>(_event_handler: F) -> BuildResult<Self> {
    Ok(Self)
  }

  fn with_config<F: FsEventHandler>(
    _event_handler: F,
    _config: FsWatcherConfig,
  ) -> BuildResult<Self> {
    Ok(Self)
  }

  fn watch(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
    Ok(())
  }

  fn unwatch(&mut self, _path: &Path) -> BuildResult<()> {
    Ok(())
  }
}
