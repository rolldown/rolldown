use oxc_resolver::{FileMetadata, FileSystem};

use crate::FileSystemExt;
use std::{
  io,
  path::{Path, PathBuf},
};

/// Operating System
#[derive(Default)]
pub struct FileSystemOs;

impl FileSystemExt for FileSystemOs {
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

impl FileSystem for FileSystemOs {
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
