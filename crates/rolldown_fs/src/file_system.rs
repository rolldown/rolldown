use std::{io, path::Path};

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
}

/// Utility trait for file system operations
pub trait FileSystemUtils: FileSystem {
  /// Empty the contents of a directory without deleting the directory itself.
  ///
  /// 1. When the path is not a directory, it will return `Err`.
  /// 2. When the path not exist, nothing will happen, it will return `Ok`.
  /// 3. Only when the path is an existing directory, it will empty inside.
  fn clean_dir(&self, path: &Path) -> io::Result<()> {
    if !self.exists(path) {
      return Ok(());
    }

    let metadata = self.metadata(path)?;
    if !metadata.is_dir() {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("not a directory: {}", path.display()),
      ));
    }

    self.remove_dir_all(path)?;
    self.create_dir_all(path)?;

    Ok(())
  }
}
