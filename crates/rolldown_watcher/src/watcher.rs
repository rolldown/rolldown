use notify::RecursiveMode;
use rolldown_error::BuildResult;
use std::path::Path;

use crate::{EventHandler, PathsMut, WatcherConfig};

pub trait Watcher {
  fn new<F: EventHandler>(event_handler: F) -> BuildResult<Self>
  where
    Self: Sized;

  fn with_config<F: EventHandler>(event_handler: F, config: WatcherConfig) -> BuildResult<Self>
  where
    Self: Sized;

  fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()>;

  /// Stop watching a path.
  ///
  /// # Errors
  ///
  /// Returns an error in the case that `path` has not been watched or if removing the watch
  /// fails.
  fn unwatch(&mut self, path: &Path) -> BuildResult<()>;

  /// Returns a mutable interface to the watched paths for batch operations.
  ///
  /// This provides better performance than multiple calls to `watch` and `unwatch`
  /// if you want to add/remove many paths at once.
  ///
  /// # Returns
  ///
  /// A boxed trait object that allows batch manipulation of watched paths.
  fn paths_mut<'me>(&'me mut self) -> Box<dyn PathsMut + 'me> {
    struct DefaultPathsMut<'a, T: ?Sized>(&'a mut T);

    impl<T: Watcher + ?Sized> PathsMut for DefaultPathsMut<'_, T> {
      fn add(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()> {
        self.0.watch(path, recursive_mode)
      }

      fn remove(&mut self, path: &Path) -> BuildResult<()> {
        self.0.unwatch(path)
      }

      fn commit(self: Box<Self>) -> BuildResult<()> {
        // No-op - changes are applied immediately
        Ok(())
      }
    }

    Box::new(DefaultPathsMut(self))
  }
}
