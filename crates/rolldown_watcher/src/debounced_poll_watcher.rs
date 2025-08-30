use std::time::Duration;

use crate::{EventHandler, Watcher, utils::DebounceEventHandlerAdapter};
use notify::PollWatcher;
use notify_debouncer_full::{Debouncer, RecommendedCache, new_debouncer_opt};
use rolldown_error::{BuildResult, ResultExt};

// We have to use newtype pattern because when we implement `Watcher` for `Debouncer<RecommendedWatcher, RecommendedCache>`.
// `RecommendedWatcher` might be  `PollWatcher` in some platforms and this will cause a compile error of duplicate implementation.
pub struct DebouncedPollWatcher(Debouncer<PollWatcher, RecommendedCache>);

impl DebouncedPollWatcher {
  pub fn with_poll_interval<F: EventHandler>(
    event_handler: F,
    poll_interval_ms: u64,
  ) -> BuildResult<Self> {
    let inner = new_debouncer_opt::<_, PollWatcher, RecommendedCache>(
      Duration::from_millis(10),
      None,
      DebounceEventHandlerAdapter(event_handler),
      RecommendedCache::new(),
      notify::Config::default().with_poll_interval(Duration::from_millis(poll_interval_ms)),
    )
    .map_err_to_unhandleable()?;

    Ok(Self(inner))
  }
}

impl Watcher for DebouncedPollWatcher {
  fn new<F: EventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    Self::with_poll_interval(event_handler, 100)
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
