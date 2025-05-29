use std::{
  io,
  path::{Path, PathBuf},
  sync::Arc,
};

use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem};
use vfs::{FileSystem as _, MemoryFS};

use crate::file_system::FileSystem;

pub type FsPath = String;
pub type FsFileContent = String;
pub type FsFileMap<'a> = &'a [(&'a FsPath, &'a FsFileContent)];

#[derive(Default, Clone)]
pub struct MemoryFileSystem {
  // root path
  fs: Arc<MemoryFS>,
}

impl MemoryFileSystem {
  /// # Panics
  ///
  /// * Fails to create directory
  /// * Fails to write file
  pub fn new(data: FsFileMap) -> Self {
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

  fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    self
      .fs
      .open_file(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::NotFound, err))?
      .read_to_end(&mut buf)?;
    Ok(buf)
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

  fn read_link(&self, _path: &Path) -> io::Result<PathBuf> {
    Err(io::Error::new(io::ErrorKind::NotFound, "not a symlink"))
  }
}

#[cfg(test)]
mod tests {
  use crate::{FileSystem as _, MemoryFileSystem};
  use oxc_resolver::FileSystem;
  use std::path::Path;

  #[test]
  pub fn test_memory_file_system() -> Result<(), String> {
    let index_path = "/index.js".to_string();
    let index_content = "const value = 1;".to_string();
    let initial_files = [(&index_path, &index_content)];
    let mut fs = MemoryFileSystem::new(&initial_files);

    let module_1_path = Path::new("/module_1.js");
    let module_1_content = "export const module_name = \"module_1\"";
    fs.add_file(module_1_path, module_1_content);

    assert_eq!(
      index_content,
      fs.read_to_string(Path::new("/index.js")).map_err(|err| err.to_string())?,
    );

    assert_eq!(
      module_1_content,
      fs.read_to_string(Path::new("/module_1.js")).map_err(|err| err.to_string())?
    );

    let ret = fs.create_dir_all(Path::new("/module_2/utils")).map_err(|err| err.kind());
    assert_eq!(Err(std::io::ErrorKind::Other), ret);

    fs.create_dir_all(Path::new("/module_2")).map_err(|err| err.to_string())?;
    fs.create_dir_all(Path::new("/module_2/utils")).map_err(|err| err.to_string())?;

    let utils_content = b"export const name = \"utils\"";
    fs.write(Path::new("/module_2/utils/index.js"), utils_content)
      .map_err(|err| err.to_string())?;

    assert_eq!(
      std::str::from_utf8(utils_content).map_err(|err| err.to_string())?,
      fs.read_to_string(Path::new("/module_2/utils/index.js")).map_err(|err| err.to_string())?
    );

    assert_eq!(
      utils_content.to_vec(),
      fs.read(Path::new("/module_2/utils/index.js")).map_err(|err| err.to_string())?
    );

    Ok(())
  }
}
