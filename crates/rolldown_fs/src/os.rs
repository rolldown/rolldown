use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem};

use std::{
  io,
  path::{Path, PathBuf},
};

use crate::file_system::FileSystem;

/// Operating System
#[derive(Default, Clone, Debug)]
pub struct OsFileSystem;

impl FileSystem for OsFileSystem {
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
}

impl OxcResolverFileSystem for OsFileSystem {
  fn read_to_string(&self, path: &Path) -> io::Result<String> {
    std::fs::read_to_string(path)
  }

  fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    std::fs::metadata(path).map(FileMetadata::from)
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    std::fs::symlink_metadata(path).map(FileMetadata::from)
  }

  fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
    dunce::canonicalize(path)
  }
}
