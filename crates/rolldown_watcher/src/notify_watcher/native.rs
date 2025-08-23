use std::time::Duration;

use crate::{EventHandler, Watcher, event::Event};
use notify::RecommendedWatcher;
use notify_debouncer_full::{
  DebounceEventHandler, DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
};
use rolldown_error::{BuildResult, ResultExt};

pub type NotifyWatcher = Debouncer<RecommendedWatcher, RecommendedCache>;
pub struct DebounceEventHandlerAdapter<T: EventHandler>(T);

impl<T> DebounceEventHandler for DebounceEventHandlerAdapter<T>
where
  T: EventHandler,
{
  fn handle_event(&mut self, event: DebounceEventResult) {
    self.0.handle_event(event.map(|events| {
      events.into_iter().map(|event| Event { detail: event.event, time: event.time }).collect()
    }));
  }
}

impl Watcher for NotifyWatcher {
  fn new<F: EventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    Ok(
      new_debouncer(Duration::from_millis(10), None, DebounceEventHandlerAdapter(event_handler))
        .map_err_to_unhandleable()?,
    )
  }

  fn watch(
    &mut self,
    path: &std::path::Path,
    recursive_mode: notify::RecursiveMode,
  ) -> BuildResult<()> {
    NotifyWatcher::watch(self, path, recursive_mode).map_err_to_unhandleable()?;

    Ok(())
  }

  fn unwatch(&mut self, path: &std::path::Path) -> BuildResult<()> {
    NotifyWatcher::unwatch(self, path).map_err_to_unhandleable()?;

    Ok(())
  }
}
