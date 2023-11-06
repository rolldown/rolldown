mod os;
mod vfs;
pub use os::*;
use oxc_resolver::FileSystem;
use std::{io, path::Path};
pub use vfs::*;

/// File System abstraction used for `ResolverGeneric`.
pub trait FileSystemExt: Send + Sync + FileSystem {
  /// # Errors
  ///
  /// * See [std::fs::remove_dir_all]
  fn remove_dir_all(&self, path: &Path) -> io::Result<()>;

  /// # Errors
  ///
  /// * See [std::fs::create_dir_all]
  fn create_dir_all(&self, path: &Path) -> io::Result<()>;

  /// # Errors
  ///
  /// * See [std::fs::write]
  fn write(&self, path: &Path, content: &[u8]) -> io::Result<()>;
}
