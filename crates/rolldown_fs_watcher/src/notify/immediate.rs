use std::{path::Path, time::Instant};

use notify::{RecursiveMode, TargetMode, WatchMode, Watcher as NotifyWatcherTrait};
use rolldown_error::{BuildResult, ResultExt};

use super::NotifyPathsMutAdapter;
use crate::{FsEvent, FsEventHandler, PathsMut, watcher::WatcherBackend};

pub(super) struct NotifyWatcher<W: NotifyWatcherTrait>(pub(super) W);

impl<W: NotifyWatcherTrait + Send> WatcherBackend for NotifyWatcher<W> {
  fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()> {
    self
      .0
      .watch(path, WatchMode { recursive_mode, target_mode: TargetMode::TrackPath })
      .map_err_to_unhandleable()?;
    Ok(())
  }

  fn unwatch(&mut self, path: &Path) -> BuildResult<()> {
    self.0.unwatch(path).map_err_to_unhandleable()?;
    Ok(())
  }

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
