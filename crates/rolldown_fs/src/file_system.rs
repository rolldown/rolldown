use std::{io, path::Path};

use oxc_resolver::FileSystem as OxcResolverFileSystem;

/// File System abstraction used for `ResolverGeneric`.
pub trait FileSystem: Send + Sync + OxcResolverFileSystem {
  /// Rolldown will access the file system from multiple places, so it's important to make sure
  /// that the file system is unique. So we use `share` to create a new reference to the same
  /// file system and this also why we don't use `Clone`. `Clone` generally means creating a new
  /// instance, but we want to share the same instance.
  #[must_use]
  fn share(&self) -> Self
  where
    Self: Sized;

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

  /// # Errors
  ///
  /// * See [std::fs::write]
  fn exists(&self, path: &Path) -> bool;
}
