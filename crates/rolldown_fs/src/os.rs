use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem, FileSystemOs};

use std::{
  io,
  path::{Path, PathBuf},
};

use crate::file_system::FileSystem;

/// Operating System
#[derive(Default, Clone, Copy, Debug)]
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

  fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
    std::fs::read(path)
  }
}

impl OxcResolverFileSystem for OsFileSystem {
  fn read_to_string(&self, path: &Path) -> io::Result<String> {
    FileSystemOs::read_to_string(path)
  }

  fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    FileSystemOs::metadata(path)
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    FileSystemOs::symlink_metadata(path)
  }

  fn read_link(&self, path: &Path) -> io::Result<PathBuf> {
    FileSystemOs::read_link(path)
  }
}
