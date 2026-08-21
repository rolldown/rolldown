use std::path::Path;

use notify::RecursiveMode;
use rolldown_error::BuildResult;

use crate::{PathsMut, watcher::WatcherBackend};

pub(super) struct NoopWatcher;

impl WatcherBackend for NoopWatcher {
  fn watch(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
    Ok(())
  }

  fn unwatch(&mut self, _path: &Path) -> BuildResult<()> {
    Ok(())
  }

  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
    Box::new(NoopPathsMut)
  }
}

struct NoopPathsMut;

impl PathsMut for NoopPathsMut {
  fn add(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
    Ok(())
  }

  fn remove(&mut self, _path: &Path) -> BuildResult<()> {
    Ok(())
  }

  fn commit(self: Box<Self>) -> BuildResult<()> {
    Ok(())
  }
}
