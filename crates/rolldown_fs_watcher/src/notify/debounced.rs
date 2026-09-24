use notify::Watcher as NotifyWatcherTrait;
use notify_debouncer_full::{
  DebounceEventHandler, DebounceEventResult, Debouncer, RecommendedCache,
};

use super::{NotifyPathsMutAdapter, PathsMut, WatcherBackend, event_map::map_notify_event};
use crate::FsEventHandler;

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
  fn handle_event(&mut self, event_result: DebounceEventResult) {
    match event_result {
      Ok(debounced_events) => {
        let mut events = Vec::new();
        for debounced_event in debounced_events {
          map_notify_event(debounced_event.event, &mut events);
        }
        if !events.is_empty() {
          self.0.handle_event(Ok(events));
        }
      }
      Err(errors) => self.0.handle_event(Err(errors)),
    }
  }
}
