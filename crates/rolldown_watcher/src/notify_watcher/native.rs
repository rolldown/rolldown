use std::time::Duration;

use crate::{EventHandler, Watcher, event::Event};
use notify::PollWatcher;
use notify_debouncer_full::{
  DebounceEventHandler, DebounceEventResult, Debouncer, RecommendedCache, new_debouncer_opt,
};
use rolldown_error::{BuildResult, ResultExt};

// FIXME: hyf0 should use recommended watcher instead of poll watcher
pub type NotifyWatcher = Debouncer<PollWatcher, RecommendedCache>;
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
    let inner = new_debouncer_opt::<_, PollWatcher, RecommendedCache>(
      Duration::from_millis(10),
      None,
      DebounceEventHandlerAdapter(event_handler),
      RecommendedCache::new(),
      notify::Config::default().with_poll_interval(Duration::from_millis(100)),
    )
    .map_err_to_unhandleable()?;

    Ok(inner)
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
