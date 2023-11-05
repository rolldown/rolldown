mod os;
mod vfs;
pub use os::*;
use oxc_resolver::{FileMetadata, FileSystem};
use std::{
  any, io,
  path::{Path, PathBuf},
};
pub use vfs::*;

/// File System abstraction used for `ResolverGeneric`.
pub trait FileSystemExt: Send + Sync {
  /// # Errors
  ///
  /// * See [std::fs::read_to_string]
  fn read_to_string_ext(&self, path: &Path) -> io::Result<String>;

  /// # Errors
  ///
  /// * See [std::fs::remove_dir_all]
  fn remove_dir_all_ext(&self, path: &Path) -> io::Result<()>;

  /// # Errors
  ///
  /// * See [std::fs::create_dir_all]
  fn create_dir_all_ext(&self, path: &Path) -> io::Result<()>;

  /// # Errors
  ///
  /// * See [std::fs::write]
  fn write_ext(&self, path: &Path, content: &[u8]) -> io::Result<()>;

  /// See [std::fs::metadata]
  ///
  /// # Errors
  ///
  /// See [std::fs::metadata]
  fn metadata_ext(&self, path: &Path) -> io::Result<FileMetadata>;

  /// See [std::fs::symlink_metadata]
  ///
  /// # Errors
  ///
  /// See [std::fs::symlink_metadata]
  fn symlink_metadata_ext(&self, path: &Path) -> io::Result<FileMetadata>;

  /// See [std::fs::canonicalize]
  ///
  /// # Errors
  ///
  /// See [std::fs::read_link]
  fn canonicalize_ext(&self, path: &Path) -> io::Result<PathBuf>;
}
