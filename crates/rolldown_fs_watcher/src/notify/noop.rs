use std::path::Path;

use notify::RecursiveMode;
use rolldown_error::BuildResult;

use super::{PathsMut, WatcherBackend};

pub(super) struct NoopWatcher;

impl WatcherBackend for NoopWatcher {
  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
    Box::new(NoopPathsMut)
  }
}

struct NoopPathsMut;

impl PathsMut for NoopPathsMut {
  fn add(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
    Ok(())
  }

  fn commit(self: Box<Self>) -> BuildResult<()> {
    Ok(())
  }
}
