use std::path::Path;

use notify::RecursiveMode;
use rolldown_error::BuildResult;

/// A trait for batch manipulation of watched paths.
///
/// This provides better performance than multiple individual calls to `watch` and `unwatch`
/// when you need to add or remove many paths at once.
pub trait PathsMut {
  /// Add a path to be watched with the specified recursive mode.
  ///
  /// # Arguments
  ///
  /// * `path` - The path to watch
  /// * `recursive_mode` - Whether to watch the path recursively or not
  ///
  /// # Returns
  ///
  /// A BuildResult indicating success or failure
  fn add(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()>;

  /// Remove a path from being watched.
  ///
  /// # Arguments
  ///
  /// * `path` - The path to stop watching
  ///
  /// # Returns
  ///
  /// A BuildResult indicating success or failure
  fn remove(&mut self, path: &Path) -> BuildResult<()>;

  /// Commit all the accumulated add/remove operations.
  ///
  /// Some implementations may apply changes immediately in `add`/`remove`,
  /// in which case this is a no-op. Others may batch all operations
  /// and apply them only when `commit` is called.
  ///
  /// # Returns
  ///
  /// A BuildResult indicating success or failure
  fn commit(self: Box<Self>) -> BuildResult<()>;
}
