mod debounced;
mod immediate;
mod noop;

use std::path::Path;

use notify::{RecursiveMode, TargetMode, WatchMode, Watcher};
use notify_debouncer_full::{RecommendedCache, new_debouncer_opt};
use rolldown_error::{BuildResult, ResultExt};

use crate::{FsEventHandler, FsWatcherConfig, PathsMut, watcher::WatcherBackend};

pub fn create_backend<F: FsEventHandler>(
  event_handler: F,
  config: &FsWatcherConfig,
) -> BuildResult<Box<dyn WatcherBackend>> {
  if !config.enabled {
    return Ok(Box::new(noop::NoopWatcher));
  }

  match (config.use_polling, config.use_debounce) {
    (true, false) => Ok(Box::new(immediate::NotifyWatcher(
      ::notify::PollWatcher::new(
        immediate::NotifyEventHandlerAdapter(event_handler),
        config.to_notify_config(),
      )
      .map_err_to_unhandleable()?,
    ))),
    (true, true) => Ok(Box::new(debounced::DebouncedNotifyWatcher(
      new_debouncer_opt::<_, ::notify::PollWatcher, RecommendedCache>(
        config.debounce_delay_duration(),
        config.debounce_tick_rate(),
        debounced::DebouncedNotifyEventHandlerAdapter(event_handler),
        RecommendedCache::new(),
        config.to_notify_config(),
      )
      .map_err_to_unhandleable()?,
    ))),
    (false, false) => Ok(Box::new(immediate::NotifyWatcher(
      ::notify::RecommendedWatcher::new(
        immediate::NotifyEventHandlerAdapter(event_handler),
        config.to_notify_config(),
      )
      .map_err_to_unhandleable()?,
    ))),
    (false, true) => Ok(Box::new(debounced::DebouncedNotifyWatcher(
      new_debouncer_opt::<_, ::notify::RecommendedWatcher, RecommendedCache>(
        config.debounce_delay_duration(),
        config.debounce_tick_rate(),
        debounced::DebouncedNotifyEventHandlerAdapter(event_handler),
        RecommendedCache::new(),
        config.to_notify_config(),
      )
      .map_err_to_unhandleable()?,
    ))),
  }
}

struct NotifyPathsMutAdapter<'me>(Box<dyn ::notify::PathsMut + 'me>);

impl<'me> NotifyPathsMutAdapter<'me> {
  pub(super) fn new(paths_mut: Box<dyn ::notify::PathsMut + 'me>) -> Self {
    Self(paths_mut)
  }
}

impl PathsMut for NotifyPathsMutAdapter<'_> {
  fn add(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()> {
    self
      .0
      .add(path, WatchMode { recursive_mode, target_mode: TargetMode::TrackPath })
      .map_err_to_unhandleable()
      .map_err(Into::into)
  }

  fn remove(&mut self, path: &Path) -> BuildResult<()> {
    self.0.remove(path).map_err_to_unhandleable().map_err(Into::into)
  }

  fn commit(self: Box<Self>) -> BuildResult<()> {
    self.0.commit().map_err_to_unhandleable().map_err(Into::into)
  }
}
