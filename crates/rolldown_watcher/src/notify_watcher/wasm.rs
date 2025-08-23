use std::time::Instant;

use crate::{EventHandler, Watcher, event::Event};
use notify::{Config, RecommendedWatcher, Watcher as NotifyWatcherTrait};
use rolldown_error::{BuildResult, ResultExt};

pub type NotifyWatcher = RecommendedWatcher;

pub struct NotifyEventHandlerAdapter<T: EventHandler>(T);

impl<T> notify::EventHandler for NotifyEventHandlerAdapter<T>
where
  T: EventHandler,
{
  fn handle_event(&mut self, event_result: notify::Result<notify::Event>) {
    let mapped = event_result
      .map_err(|err| vec![err])
      .map(|evt| vec![Event { detail: evt, time: Instant::now() }]);

    self.0.handle_event(mapped);
  }
}

impl Watcher for NotifyWatcher {
  fn new<F: EventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    let watcher = <NotifyWatcher as NotifyWatcherTrait>::new(
      NotifyEventHandlerAdapter(event_handler),
      Config::default(),
    )
    .map_err_to_unhandleable()?;

    Ok(watcher)
  }

  fn watch(
    &mut self,
    path: &std::path::Path,
    recursive_mode: notify::RecursiveMode,
  ) -> BuildResult<()> {
    NotifyWatcherTrait::watch(self, path, recursive_mode).map_err_to_unhandleable()?;
    Ok(())
  }

  fn unwatch(&mut self, path: &std::path::Path) -> BuildResult<()> {
    NotifyWatcherTrait::unwatch(self, path).map_err_to_unhandleable()?;
    Ok(())
  }
}
