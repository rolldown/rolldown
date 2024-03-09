use std::{
  io,
  path::{Path, PathBuf},
  sync::Arc,
};

use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem};
use vfs::{FileSystem as _, MemoryFS};

use crate::file_system::FileSystem;

#[derive(Default)]
pub struct MemoryFileSystem {
  // root path
  fs: Arc<MemoryFS>,
}

impl MemoryFileSystem {
  /// # Panics
  ///
  /// * Fails to create directory
  /// * Fails to write file
  pub fn new(data: &[(&String, &String)]) -> Self {
    let mut fs = Self::default();
    for (path, content) in data {
      fs.add_file(Path::new(path), content);
    }
    fs
  }

  pub fn add_file(&mut self, path: &Path, content: &str) {
    let fs = &mut self.fs;
    // Create all parent directories
    for path in path.ancestors().collect::<Vec<_>>().iter().rev() {
      let path = path.to_string_lossy();
      if !fs.exists(path.as_ref()).unwrap() {
        fs.create_dir(path.as_ref()).unwrap();
      }
    }
    // Create file
    let mut file = fs.create_file(path.to_string_lossy().as_ref()).unwrap();
    file.write_all(content.as_bytes()).unwrap();
  }
}

impl FileSystem for MemoryFileSystem {
  fn share(&self) -> Self {
    Self { fs: Arc::clone(&self.fs) }
  }

  fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
    self
      .fs
      .remove_dir(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
  }

  fn create_dir_all(&self, path: &Path) -> io::Result<()> {
    self
      .fs
      .create_dir(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
  }

  fn write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
    _ = self
      .fs
      .create_file(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
      .write(content)
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    Ok(())
  }

  fn exists(&self, path: &Path) -> bool {
    self.fs.exists(path.to_string_lossy().as_ref()).is_ok()
  }
}

impl OxcResolverFileSystem for MemoryFileSystem {
  fn read_to_string(&self, path: &Path) -> io::Result<String> {
    let mut buf = String::new();
    self
      .fs
      .open_file(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::NotFound, err))?
      .read_to_string(&mut buf)?;
    Ok(buf)
  }

  fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    let metadata = self
      .fs
      .metadata(path.to_string_lossy().as_ref())
      .map_err(|err| io::Error::new(io::ErrorKind::NotFound, err))?;
    let is_file = metadata.file_type == vfs::VfsFileType::File;
    let is_dir = metadata.file_type == vfs::VfsFileType::Directory;
    Ok(FileMetadata::new(is_file, is_dir, false))
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    self.metadata(path).map_err(|err| {
      io::Error::new(io::ErrorKind::NotFound, format!("symlink_metadata failed: {err}"))
    })
  }

  fn canonicalize(&self, _path: &Path) -> io::Result<PathBuf> {
    Err(io::Error::new(io::ErrorKind::NotFound, "not a symlink"))
  }
}
