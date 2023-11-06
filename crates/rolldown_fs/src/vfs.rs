use std::{
  io,
  path::{Path, PathBuf},
};

use oxc_resolver::{FileMetadata, FileSystem};
use vfs::MemoryFS;

use crate::FileSystemExt;

pub struct FileSystemVfs {
  // root path
  fs: MemoryFS,
}

impl Default for FileSystemVfs {
  fn default() -> Self {
    let fs = vfs::MemoryFS::new();
    Self { fs }
  }
}

impl FileSystemExt for FileSystemVfs {
  fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
    use vfs::FileSystem;
    self
      .fs
      .remove_dir(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    Ok(())
  }

  fn create_dir_all(&self, path: &Path) -> io::Result<()> {
    use vfs::FileSystem;
    self
      .fs
      .create_dir(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    Ok(())
  }

  fn write(&self, path: &Path, content: &[u8]) -> io::Result<()> {
    use vfs::FileSystem;
    _ = self
      .fs
      .create_file(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
      .write(content)
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    Ok(())
  }
}

impl FileSystem for FileSystemVfs {
  fn read_to_string(&self, path: &Path) -> io::Result<String> {
    use vfs::FileSystem;
    let mut buf = String::new();
    self
      .fs
      .open_file(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::NotFound, err))?
      .read_to_string(&mut buf)?;
    Ok(buf)
  }

  fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    use vfs::FileSystem;
    let metadata = self
      .fs
      .metadata(path.to_string_lossy().as_ref())
      .map_err(|err| io::Error::new(io::ErrorKind::NotFound, err))?;
    let is_file = metadata.file_type == vfs::VfsFileType::File;
    let is_dir = metadata.file_type == vfs::VfsFileType::Directory;
    Ok(FileMetadata::new(is_file, is_dir, false))
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    use vfs::FileSystem;
    self
      .fs
      .metadata(path.to_string_lossy().as_ref())
      .map(|meta| {
        let is_file = meta.file_type == vfs::VfsFileType::File;
        let is_dir = meta.file_type == vfs::VfsFileType::Directory;
        FileMetadata::new(is_file, is_dir, false)
      })
      .map_err(|err| {
        io::Error::new(io::ErrorKind::NotFound, format!("symlink_metadata failed: {err}"))
      })
  }

  fn canonicalize(&self, _path: &Path) -> io::Result<PathBuf> {
    Err(io::Error::new(io::ErrorKind::NotFound, "not a symlink"))
  }
}
