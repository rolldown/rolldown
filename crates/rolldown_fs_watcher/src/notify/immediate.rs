use std::time::Instant;

use notify::Watcher as NotifyWatcherTrait;

use super::NotifyPathsMutAdapter;
use crate::{
  FsEvent, FsEventHandler,
  watcher::{PathsMut, WatcherBackend},
};

pub(super) struct NotifyWatcher<W: NotifyWatcherTrait>(pub(super) W);

impl<W: NotifyWatcherTrait + Send> WatcherBackend for NotifyWatcher<W> {
  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
    Box::new(NotifyPathsMutAdapter::new(self.0.paths_mut()))
  }
}

pub(super) struct NotifyEventHandlerAdapter<T: FsEventHandler>(pub(super) T);

impl<T: FsEventHandler> ::notify::EventHandler for NotifyEventHandlerAdapter<T> {
  fn handle_event(&mut self, event_result: ::notify::Result<::notify::Event>) {
    let event = event_result
      .map_err(|error| vec![error])
      .map(|event| vec![FsEvent { detail: event, time: Instant::now() }]);
    self.0.handle_event(event);
  }
}
