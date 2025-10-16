use std::{
  io,
  path::{Path, PathBuf},
};

use oxc_resolver::FileSystem as OxcResolverFileSystem;

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
  /// * See [std::path::Path::exists]
  fn exists(&self, path: &Path) -> bool;

  /// # Errors
  ///
  /// * See [std::fs::read]
  fn read(&self, path: &Path) -> io::Result<Vec<u8>>;

  /// Returns a vector of absolute paths to the directory entries.
  ///
  /// Here we don't return [`std::fs::ReadDir`] because
  /// it's inside the [`std::fs`] module, which might incompatible
  /// with the in-memory mode.
  ///
  /// * See [std::fs::read_dir]
  fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;

  /// # Errors
  ///
  /// * See [std::fs::remove_file]
  fn remove_file(&self, path: &Path) -> io::Result<()>;
}
