use notify::Watcher as NotifyWatcherTrait;

use super::NotifyPathsMutAdapter;
use crate::{
  FsEventHandler,
  event_map::map_notify_event,
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
    match event_result {
      Ok(event) => {
        let mut events = Vec::new();
        map_notify_event(event, &mut events);
        if !events.is_empty() {
          self.0.handle_event(Ok(events));
        }
      }
      Err(error) => self.0.handle_event(Err(vec![error])),
    }
  }
}
