use crate::{
  FsEventHandler, FsWatcher, FsWatcherConfig,
  utils::{NotifyEventHandlerAdapter, NotifyPathsMutAdapter},
};
use notify::{RecommendedWatcher as NotifyRecommendedWatcher, Watcher as NotifyWatcherTrait};
use rolldown_error::{BuildResult, ResultExt};

/// Will use the ideal filesystem watcher under the hood based on the platform.
pub struct RecommendedFsWatcher(NotifyRecommendedWatcher);

impl FsWatcher for RecommendedFsWatcher {
  fn new<F: FsEventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized,
  {
    Self::with_config(event_handler, FsWatcherConfig::default())
  }

  fn with_config<F: FsEventHandler>(event_handler: F, config: FsWatcherConfig) -> BuildResult<Self>
  where
    Self: Sized,
  {
    let watcher = <NotifyRecommendedWatcher as NotifyWatcherTrait>::new(
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

  fn paths_mut<'me>(&'me mut self) -> Box<dyn crate::PathsMut + 'me> {
    let paths_mut = self.0.paths_mut();
    Box::new(NotifyPathsMutAdapter::new(paths_mut))
  }
}
