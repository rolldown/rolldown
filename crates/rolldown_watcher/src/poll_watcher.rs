use crate::{EventHandler, Watcher, WatcherConfig, utils::NotifyEventHandlerAdapter};
use notify::{PollWatcher as NotifyPollWatcher, Watcher as NotifyWatcherTrait};
use rolldown_error::{BuildResult, ResultExt};

/// A non-debounced polling-based watcher that checks for file changes at regular intervals.
pub struct PollWatcher(NotifyPollWatcher);

impl Watcher for PollWatcher {
  fn new<F: EventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    Self::with_config(event_handler, WatcherConfig::default())
  }

  fn with_config<F: EventHandler>(event_handler: F, config: WatcherConfig) -> BuildResult<Self>
  where
    Self: Sized,
  {
    let watcher = <NotifyPollWatcher as NotifyWatcherTrait>::new(
      NotifyEventHandlerAdapter(event_handler),
      config.to_notify_config(),
    )
    .map_err_to_unhandleable()?;

    Ok(Self(watcher))
  }

  fn watch(
    &mut self,
    path: &std::path::Path,
    recursive_mode: notify::RecursiveMode,
  ) -> BuildResult<()> {
    self.0.watch(path, recursive_mode).map_err_to_unhandleable()?;
    Ok(())
  }

  fn unwatch(&mut self, path: &std::path::Path) -> BuildResult<()> {
    self.0.unwatch(path).map_err_to_unhandleable()?;
    Ok(())
  }
}
