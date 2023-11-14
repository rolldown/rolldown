use std::{io, path::Path, sync::Arc};

use oxc_resolver::FileSystem as OxcResolverFileSystem;

/// File System abstraction used for `ResolverGeneric`.
pub trait FileSystem: Send + Sync + OxcResolverFileSystem {
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

impl<T> FileSystem for Arc<T>
where
  T: FileSystem + OxcResolverFileSystem,
{
  fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
    self.as_ref().remove_dir_all(path)
  }

  fn create_dir_all(&self, path: &Path) -> io::Result<()> {
    self.as_ref().create_dir_all(path)
  }

  fn write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
    self.as_ref().write(path, content)
  }

  fn exists(&self, path: &Path) -> bool {
    self.as_ref().exists(path)
  }
}
