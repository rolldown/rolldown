use notify::Watcher as NotifyWatcherTrait;
use notify_debouncer_full::{
  DebounceEventHandler, DebounceEventResult, Debouncer, RecommendedCache,
};

use super::{NotifyEventHandlerAdapter, NotifyPathsMutAdapter, PathsMut, WatcherBackend};
use crate::FsEventHandler;

pub(super) struct DebouncedNotifyWatcher<W: NotifyWatcherTrait>(
  pub(super) Debouncer<W, RecommendedCache>,
);

impl<W: NotifyWatcherTrait + Send> WatcherBackend for DebouncedNotifyWatcher<W> {
  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
    Box::new(NotifyPathsMutAdapter::new(self.0.paths_mut()))
  }
}

impl<T: FsEventHandler> DebounceEventHandler for NotifyEventHandlerAdapter<T> {
  fn handle_event(&mut self, event_result: DebounceEventResult) {
    match event_result {
      Ok(debounced_events) => self.deliver(debounced_events.into_iter().map(|event| event.event)),
      Err(errors) => self.0.handle_event(Err(errors)),
    }
  }
}
