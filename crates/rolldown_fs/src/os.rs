use std::{
  fmt, io,
  path::{Path, PathBuf},
  sync::Arc,
};

use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem, FileSystemOs};

use crate::file_system::FileSystem;

/// Operating System
#[derive(Clone)]
pub struct OsFileSystem(Arc<FileSystemOs>);

impl fmt::Debug for OsFileSystem {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "OsFileSystem")
  }
}

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
  fn new(yarn_pnp: bool) -> Self {
    Self(Arc::new(FileSystemOs::new(yarn_pnp)))
  }

  fn read_to_string(&self, path: &Path) -> io::Result<String> {
    self.0.read_to_string(path)
  }

  fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    self.0.metadata(path)
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    self.0.symlink_metadata(path)
  }

  fn read_link(&self, path: &Path) -> io::Result<PathBuf> {
    self.0.read_link(path)
  }
}
