use crate::{
  EventHandler, Watcher, WatcherConfig,
  utils::{DebounceEventHandlerAdapter, NotifyPathsMutAdapter},
};
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
    Self::with_config(event_handler, WatcherConfig::default())
  }

  fn with_config<F: EventHandler>(event_handler: F, config: WatcherConfig) -> BuildResult<Self>
  where
    Self: Sized,
  {
    let inner = new_debouncer_opt::<_, RecommendedWatcher, RecommendedCache>(
      config.debounce_delay_duration(),
      config.debounce_tick_rate(),
      DebounceEventHandlerAdapter(event_handler),
      RecommendedCache::new(),
      config.to_notify_config(),
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

  fn paths_mut<'me>(&'me mut self) -> Box<dyn crate::PathsMut + 'me> {
    let paths_mut = self.0.paths_mut();
    Box::new(NotifyPathsMutAdapter::new(paths_mut))
  }
}
