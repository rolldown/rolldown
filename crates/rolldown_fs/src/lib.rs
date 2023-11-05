mod os;
mod vfs;
pub use os::*;
use std::path::Path;
pub use vfs::*;

/// File System abstraction used for `ResolverGeneric`.
pub trait FileSystem: Send + Sync {
  /// See [std::fs::read_to_string]
  ///
  /// # Errors
  ///
  /// * See [std::fs::read_to_string]
  fn read_to_string(&self, path: &Path) -> anyhow::Result<String>;
}
