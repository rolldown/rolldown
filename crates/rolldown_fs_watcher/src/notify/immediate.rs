use notify::Watcher as NotifyWatcherTrait;

use super::{NotifyEventHandlerAdapter, NotifyPathsMutAdapter, PathsMut, WatcherBackend};
use crate::FsEventHandler;

pub(super) struct NotifyWatcher<W: NotifyWatcherTrait>(pub(super) W);

impl<W: NotifyWatcherTrait + Send> WatcherBackend for NotifyWatcher<W> {
  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
    Box::new(NotifyPathsMutAdapter::new(self.0.paths_mut()))
  }
}

impl<T: FsEventHandler> ::notify::EventHandler for NotifyEventHandlerAdapter<T> {
  fn handle_event(&mut self, event_result: ::notify::Result<::notify::Event>) {
    match event_result {
      Ok(event) => self.deliver([event]),
      Err(error) => tracing::error!("notify error: {error:?}"),
    }
  }
}
