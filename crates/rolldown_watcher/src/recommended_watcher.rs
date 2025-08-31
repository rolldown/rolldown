use crate::{EventHandler, Watcher, utils::NotifyEventHandlerAdapter};
use notify::{
  Config, RecommendedWatcher as NotifyRecommendedWatcher, Watcher as NotifyWatcherTrait,
};
use rolldown_error::{BuildResult, ResultExt};

/// Will use the ideal watcher under the hood based on the platform.
pub struct RecommendedWatcher(NotifyRecommendedWatcher);

impl Watcher for RecommendedWatcher {
  fn new<F: EventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    let watcher = <NotifyRecommendedWatcher as NotifyWatcherTrait>::new(
      NotifyEventHandlerAdapter(event_handler),
      Config::default(),
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
