use std::{
  io,
  path::{Path, PathBuf},
};

use crate::file_system::FileSystem;

/// Operating system file system — oxc-resolver's [`FileSystemOs`], re-exported.
///
/// rolldown's write-capable [`FileSystem`] trait is implemented directly on it below — allowed by
/// the orphan rule because the trait is local to this crate. Using oxc-resolver's type directly
/// (rather than a newtype wrapper) means rolldown's bundler resolver and oxc-resolver share a
/// single `Fs` type instead of two.
pub use oxc_resolver::FileSystemOs;

impl FileSystem for FileSystemOs {
  fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
    std::fs::remove_dir_all(path)
  }

  fn create_dir_all(&self, path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
  }

  fn write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
    std::fs::write(path, content)
  }

  fn exists(&self, path: &Path) -> bool {
    path.exists()
  }

  fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(path)?;
    let mut paths = Vec::new();
    for entry in entries {
      let entry = entry?;
      paths.push(entry.path());
    }
    Ok(paths)
  }

  fn remove_file(&self, path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)
  }
}
