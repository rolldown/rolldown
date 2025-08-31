use std::time::Duration;

use crate::{EventHandler, Watcher, utils::DebounceEventHandlerAdapter};
use notify::RecommendedWatcher;
use notify_debouncer_full::{Debouncer, RecommendedCache, new_debouncer_opt};
use rolldown_error::{BuildResult, ResultExt};

// We have to use newtype pattern because when we implement `Watcher` for `Debouncer<RecommendedWatcher, RecommendedCache>`.
// `RecommendedWatcher` might be `PollWatcher` in some platforms and this will cause a compile error of duplicate implementation.
pub struct DebouncedRecommendedWatcher(Debouncer<RecommendedWatcher, RecommendedCache>);

impl Watcher for DebouncedRecommendedWatcher {
  fn new<F: EventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    let inner = new_debouncer_opt::<_, RecommendedWatcher, RecommendedCache>(
      Duration::from_millis(10),
      None,
      DebounceEventHandlerAdapter(event_handler),
      RecommendedCache::new(),
      notify::Config::default(),
    )
    .map_err_to_unhandleable()?;

    Ok(Self(inner))
  }

  fn watch(
    &mut self,
    path: &std::path::Path,
    recursive_mode: notify::RecursiveMode,
  ) -> BuildResult<()> {
    self.0.watch(path, recursive_mode).map_err_to_unhandleable()?;

    Ok(())
  }

  fn unwatch(&mut self, path: &std::path::Path) -> BuildResult<()> {
    self.0.unwatch(path).map_err_to_unhandleable()?;
    Ok(())
  }
}
