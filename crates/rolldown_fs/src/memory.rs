use std::{
  error::Error,
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
  pub fn new(data: &[(&String, &String)]) -> Result<Self, Box<dyn Error>> {
    let mut fs = Self::default();
    for (path, content) in data {
      fs.add_file(Path::new(path), content)?;
    }
    Ok(fs)
  }

  pub fn add_file(&mut self, path: &Path, content: &str) -> Result<(), Box<dyn Error>> {
    let fs = &mut self.fs;

    // Create all parent directories
    path
      .ancestors()
      .collect::<Vec<_>>()
      .into_iter()
      .rev()
      .map(|p| p.to_string_lossy().into_owned())
      .try_for_each(|p| match fs.exists(p.as_ref()) {
        Ok(true) => Ok(()),
        Ok(false) => fs.create_dir(p.as_ref()).map_err(|_| "Failed to create directory"),
        Err(_) => Err("Failed to check if directory exists"),
      })?;

    // Create file
    let mut file = fs.create_file(path.to_string_lossy().as_ref())?;
    file.write_all(content.as_bytes())?;
    Ok(())
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
