mod debounced;
mod event_map;
mod immediate;
mod noop;

use std::{path::Path, sync::Arc};

use notify::{Event, RecursiveMode, TargetMode, WatchMode, Watcher};
use notify_debouncer_full::{RecommendedCache, new_debouncer_opt};
use rolldown_error::{BuildResult, ResultExt};

use crate::{FsEventHandler, FsWatcherConfig, filter::IgnoreFilter};

pub trait WatcherBackend: Send {
  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_>;
}

pub trait PathsMut {
  fn add(&mut self, path: &Path) -> BuildResult<()>;

  fn commit(self: Box<Self>) -> BuildResult<()>;
}

pub fn create_backend<F: FsEventHandler>(
  event_handler: F,
  config: &FsWatcherConfig,
  filter: Option<Arc<IgnoreFilter>>,
) -> BuildResult<Box<dyn WatcherBackend>> {
  if !config.enabled {
    return Ok(Box::new(noop::NoopWatcher));
  }

  let notify_config = config.to_notify_config(filter.clone());
  Ok(match (config.use_polling, config.use_debounce) {
    (true, false) => Box::new(immediate::NotifyWatcher(
      ::notify::PollWatcher::new(
        NotifyEventHandlerAdapter { handler: event_handler, filter },
        notify_config,
      )
      .map_err_to_unhandleable()?,
    )),
    (true, true) => Box::new(debounced::DebouncedNotifyWatcher(
      new_debouncer_opt::<_, ::notify::PollWatcher, RecommendedCache>(
        config.debounce_delay_duration(),
        config.debounce_tick_rate(),
        NotifyEventHandlerAdapter { handler: event_handler, filter },
        RecommendedCache::new(),
        notify_config,
      )
      .map_err_to_unhandleable()?,
    )),
    (false, false) => Box::new(immediate::NotifyWatcher(
      ::notify::RecommendedWatcher::new(
        NotifyEventHandlerAdapter { handler: event_handler, filter },
        notify_config,
      )
      .map_err_to_unhandleable()?,
    )),
    (false, true) => Box::new(debounced::DebouncedNotifyWatcher(
      new_debouncer_opt::<_, ::notify::RecommendedWatcher, RecommendedCache>(
        config.debounce_delay_duration(),
        config.debounce_tick_rate(),
        NotifyEventHandlerAdapter { handler: event_handler, filter },
        RecommendedCache::new(),
        notify_config,
      )
      .map_err_to_unhandleable()?,
    )),
  })
}

struct NotifyPathsMutAdapter<'me>(Box<dyn ::notify::PathsMut + 'me>);

impl<'me> NotifyPathsMutAdapter<'me> {
  pub(super) fn new(paths_mut: Box<dyn ::notify::PathsMut + 'me>) -> Self {
    Self(paths_mut)
  }
}

impl PathsMut for NotifyPathsMutAdapter<'_> {
  fn add(&mut self, path: &Path) -> BuildResult<()> {
    self
      .0
      .add(
        path,
        WatchMode { recursive_mode: RecursiveMode::Recursive, target_mode: TargetMode::TrackPath },
      )
      .map_err_to_unhandleable()
      .map_err(Into::into)
  }

  fn commit(self: Box<Self>) -> BuildResult<()> {
    self.0.commit().map_err_to_unhandleable().map_err(Into::into)
  }
}

struct NotifyEventHandlerAdapter<T> {
  handler: T,
  filter: Option<Arc<IgnoreFilter>>,
}

impl<T: FsEventHandler> NotifyEventHandlerAdapter<T> {
  fn deliver(&mut self, notify_events: impl IntoIterator<Item = Event>) {
    let mut events = Vec::new();
    for notify_event in notify_events {
      event_map::map_notify_event(notify_event, self.filter.as_deref(), &mut events);
    }
    if !events.is_empty() {
      self.handler.handle_events(events);
    }
  }
}
