use std::time::Duration;

use notify::RecommendedWatcher;
use notify_debouncer_full::{DebounceEventHandler, Debouncer, RecommendedCache, new_debouncer};
use rolldown_error::{BuildResult, ResultExt};

use crate::Watcher;

pub type NotifyWatcher = Debouncer<RecommendedWatcher, RecommendedCache>;

impl Watcher for NotifyWatcher {
  fn new<F: DebounceEventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    Ok(new_debouncer(Duration::from_millis(10), None, event_handler).map_err_to_unhandleable()?)
  }

  fn watch(
    &mut self,
    path: &std::path::Path,
    recursive_mode: notify::RecursiveMode,
  ) -> BuildResult<()> {
    NotifyWatcher::watch(self, path, recursive_mode).map_err_to_unhandleable()?;

    Ok(())
  }

  fn unwatch(&mut self, path: &std::path::Path) -> BuildResult<()> {
    NotifyWatcher::unwatch(self, path).map_err_to_unhandleable()?;

    Ok(())
  }
}
