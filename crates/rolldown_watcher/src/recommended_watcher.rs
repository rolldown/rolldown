use std::path::Path;

use crate::{EventHandler, Watcher, WatcherConfig, utils::NotifyEventHandlerAdapter};
use notify::{
  RecommendedWatcher as NotifyRecommendedWatcher, RecursiveMode, Watcher as NotifyWatcherTrait,
};
use rolldown_error::{BuildResult, ResultExt};

/// Will use the ideal watcher under the hood based on the platform.
pub struct RecommendedWatcher(NotifyRecommendedWatcher);

impl Watcher for RecommendedWatcher {
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
    struct CustomPathsMut<'me>(Box<dyn notify::PathsMut + 'me>);

    impl crate::PathsMut for CustomPathsMut<'_> {
      fn add(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()> {
        self.0.add(path, recursive_mode).map_err_to_unhandleable().map_err(Into::into)
      }

      fn remove(&mut self, path: &Path) -> BuildResult<()> {
        self.0.remove(path).map_err_to_unhandleable().map_err(Into::into)
      }

      fn commit(self: Box<Self>) -> BuildResult<()> {
        self.0.commit().map_err_to_unhandleable().map_err(Into::into)
      }
    }

    let paths_mut = self.0.paths_mut();
    Box::new(CustomPathsMut(paths_mut))
  }
}
