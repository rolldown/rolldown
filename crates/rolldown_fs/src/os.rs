use oxc_resolver::{FileMetadata, FileSystem};

use crate::FileSystemExt;
use std::{
  fs, io,
  path::{Path, PathBuf},
};

/// Operating System
#[derive(Default)]
pub struct FileSystemOs;

impl FileSystemExt for FileSystemOs {
  fn read_to_string_ext(&self, path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
  }

  fn remove_dir_all_ext(&self, path: &Path) -> io::Result<()> {
    std::fs::remove_dir_all(path)
  }

  fn create_dir_all_ext(&self, path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)
  }

  fn write_ext(&self, path: &Path, content: &[u8]) -> io::Result<()> {
    std::fs::write(path, content)
  }

  fn metadata_ext(&self, path: &Path) -> io::Result<FileMetadata> {
    std::fs::metadata(path).map(FileMetadata::from)
  }

  fn symlink_metadata_ext(&self, path: &Path) -> io::Result<FileMetadata> {
    std::fs::symlink_metadata(path).map(FileMetadata::from)
  }

  fn canonicalize_ext(&self, path: &Path) -> io::Result<PathBuf> {
    dunce::canonicalize(path)
  }
}

impl FileSystem for FileSystemOs {
  fn read_to_string<P: AsRef<Path>>(&self, path: P) -> io::Result<String> {
    self.read_to_string_ext(path.as_ref())
  }

  fn metadata<P: AsRef<Path>>(&self, path: P) -> io::Result<FileMetadata> {
    self.metadata_ext(path.as_ref())
  }

  fn symlink_metadata<P: AsRef<Path>>(&self, path: P) -> io::Result<FileMetadata> {
    self.symlink_metadata_ext(path.as_ref())
  }

  fn canonicalize<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
    self.canonicalize_ext(path.as_ref())
  }
}
