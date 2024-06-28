use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem};

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
    std::fs::read_to_string(path)
  }

  fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    std::fs::metadata(path).map(FileMetadata::from)
  }

  fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
    std::fs::symlink_metadata(path).map(FileMetadata::from)
  }

  fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
    #[cfg(not(target_os = "wasi"))]
    {
      dunce::canonicalize(path)
    }
    #[cfg(target_os = "wasi")]
    {
      let meta = std::fs::symlink_metadata(path)?;
      if meta.file_type().is_symlink() {
        let link = std::fs::read_link(path)?;
        let mut path_buf = path.to_path_buf();
        path_buf.pop();
        for segment in link.iter() {
          match segment.to_str() {
            Some("..") => {
              path_buf.pop();
            }
            Some(".") | None => {}
            Some(seg) => {
              // Need to trim the extra \0 introduces by rust std rust-lang/rust#123727
              path_buf.push(seg.trim_end_matches('\0'));
            }
          }
        }
        Ok(path_buf)
      } else {
        Ok(path.to_path_buf())
      }
    }
  }
}
