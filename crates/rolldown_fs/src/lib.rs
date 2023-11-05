mod os;
mod vfs;
pub use os::*;
use std::path::Path;
pub use vfs::*;

/// File System abstraction used for `ResolverGeneric`.
pub trait FileSystem: Send + Sync {
  /// # Errors
  ///
  /// * See [std::fs::read_to_string]
  fn read_to_string(&self, path: &Path) -> anyhow::Result<String>;

  /// # Errors
  ///
  /// * See [std::fs::remove_dir_all]
  fn remove_dir_all(&self, path: &Path) -> anyhow::Result<()>;

  /// # Errors
  ///
  /// * See [std::fs::create_dir_all]
  fn create_dir_all(&self, path: &Path) -> anyhow::Result<()>;

  /// # Errors
  ///
  /// * See [std::fs::write]
  fn write(&self, path: &Path, content: &[u8]) -> anyhow::Result<()>;
}
