use notify::Watcher as NotifyWatcherTrait;

use super::NotifyPathsMutAdapter;
use crate::{
  FsEventHandler,
  event_map::EventMapper,
  watcher::{PathsMut, WatcherBackend},
};

pub(super) struct NotifyWatcher<W: NotifyWatcherTrait>(pub(super) W);

impl<W: NotifyWatcherTrait + Send> WatcherBackend for NotifyWatcher<W> {
  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
    Box::new(NotifyPathsMutAdapter::new(self.0.paths_mut()))
  }
}

pub(super) struct NotifyEventHandlerAdapter<T: FsEventHandler> {
  pub(super) handler: T,
  pub(super) mapper: EventMapper,
}

impl<T: FsEventHandler> ::notify::EventHandler for NotifyEventHandlerAdapter<T> {
  fn handle_event(&mut self, event_result: ::notify::Result<::notify::Event>) {
    match event_result {
      Ok(event) => {
        let mut events = Vec::new();
        self.mapper.map(event, &mut events);
        if !events.is_empty() {
          self.handler.handle_event(Ok(events));
        }
      }
      Err(error) => self.handler.handle_event(Err(vec![error])),
    }
  }
}
