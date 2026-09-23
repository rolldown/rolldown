use notify::Watcher as NotifyWatcherTrait;
use notify_debouncer_full::{
  DebounceEventHandler, DebounceEventResult, Debouncer, RecommendedCache,
};

use super::NotifyPathsMutAdapter;
use crate::{
  FsEvent, FsEventHandler,
  watcher::{PathsMut, WatcherBackend},
};

pub(super) struct DebouncedNotifyWatcher<W: NotifyWatcherTrait>(
  pub(super) Debouncer<W, RecommendedCache>,
);

impl<W: NotifyWatcherTrait + Send> WatcherBackend for DebouncedNotifyWatcher<W> {
  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
    Box::new(NotifyPathsMutAdapter::new(self.0.paths_mut()))
  }
}

pub(super) struct DebouncedNotifyEventHandlerAdapter<T: FsEventHandler>(pub(super) T);

impl<T: FsEventHandler> DebounceEventHandler for DebouncedNotifyEventHandlerAdapter<T> {
  fn handle_event(&mut self, event: DebounceEventResult) {
    self.0.handle_event(event.map(|events| {
      events.into_iter().map(|event| FsEvent { detail: event.event, time: event.time }).collect()
    }));
  }
}
