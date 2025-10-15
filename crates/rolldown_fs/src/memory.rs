use std::{
  io,
  path::{Path, PathBuf},
  sync::Arc,
};

use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem, ResolveError};
use vfs::{FileSystem as _, MemoryFS};

use crate::file_system::{FileSystem, FileSystemUtils};

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

  fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
    let path_str = path.to_string_lossy();
    let entries = self
      .fs
      .read_dir(path_str.as_ref())
      .map_err(|err| io::Error::new(io::ErrorKind::NotFound, err))?;

    let mut paths = Vec::new();
    for entry in entries {
      let entry_path = PathBuf::from(entry);
      paths.push(entry_path);
    }
    Ok(paths)
  }

  fn remove_file(&self, path: &Path) -> io::Result<()> {
    self
      .fs
      .remove_file(&path.to_string_lossy())
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
  }
}

impl FileSystemUtils for MemoryFileSystem {}

impl OxcResolverFileSystem for MemoryFileSystem {
  fn new(_yarn_pnp: bool) -> Self {
    Self::default()
  }

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

  fn read_link(&self, _path: &Path) -> Result<PathBuf, ResolveError> {
    Err(ResolveError::from(io::Error::new(io::ErrorKind::NotFound, "not a symlink")))
  }
}

#[cfg(test)]
mod tests {
  use crate::{FileSystem as _, MemoryFileSystem};
  use oxc_resolver::FileSystem;
  use std::path::Path;

  #[test]
  fn test_memory_file_system() -> Result<(), String> {
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

  #[cfg(test)]
  mod clean_dir {
    use super::*;
    use crate::file_system::FileSystemUtils;

    #[test]
    fn non_existent_dir() -> Result<(), String> {
      let fs = MemoryFileSystem::new(&[]);
      fs.clean_dir(Path::new("/non_existent")).map_err(|err| err.to_string())?;
      Ok(())
    }

    #[test]
    fn empty_dir() -> Result<(), String> {
      let fs = MemoryFileSystem::new(&[]);
      fs.create_dir_all(Path::new("/empty_dir")).map_err(|err| err.to_string())?;
      assert!(fs.exists(Path::new("/empty_dir")));
      fs.clean_dir(Path::new("/empty_dir")).map_err(|err| err.to_string())?;
      assert!(fs.exists(Path::new("/empty_dir")));
      Ok(())
    }

    #[test]
    fn clean_file_should_fail() -> Result<(), String> {
      let fs = MemoryFileSystem::new(&[]);
      fs.write(Path::new("/test_file.txt"), b"content").map_err(|err| err.to_string())?;
      let result = fs.clean_dir(Path::new("/test_file.txt"));
      assert!(result.is_err());
      Ok(())
    }

    #[test]
    fn files_and_sub_dirs() -> Result<(), String> {
      let fs = MemoryFileSystem::new(&[]);

      // Create directory structure.
      fs.create_dir_all(Path::new("/test_dir")).map_err(|err| err.to_string())?;
      fs.write(Path::new("/test_dir/file1.txt"), b"content1").map_err(|err| err.to_string())?;
      fs.write(Path::new("/test_dir/file2.txt"), b"content2").map_err(|err| err.to_string())?;
      let subdir = Path::new("/test_dir/subdir");
      fs.create_dir_all(subdir).map_err(|err| err.to_string())?;
      fs.write(Path::new("/test_dir/subdir/file3.txt"), b"content3")
        .map_err(|err| err.to_string())?;

      // Verify files exist before cleaning.
      assert!(fs.exists(Path::new("/test_dir")));
      assert!(fs.exists(Path::new("/test_dir/file1.txt")));
      assert!(fs.exists(Path::new("/test_dir/file2.txt")));
      assert!(fs.exists(subdir));
      assert!(fs.exists(Path::new("/test_dir/subdir/file3.txt")));

      fs.clean_dir(Path::new("/test_dir")).map_err(|err| err.to_string())?;
      assert!(fs.exists(Path::new("/test_dir")));
      assert!(!fs.exists(Path::new("/test_dir/file1.txt")));
      assert!(!fs.exists(Path::new("/test_dir/file2.txt")));
      assert!(!fs.exists(subdir));

      Ok(())
    }
  }

  #[cfg(test)]
  mod read_dir {
    use super::*;

    #[test]
    fn basic() -> Result<(), String> {
      let fs = MemoryFileSystem::new(&[]);

      fs.create_dir_all(Path::new("/test_dir")).map_err(|err| err.to_string())?;
      fs.write(Path::new("/test_dir/file1.js"), b"content1").map_err(|err| err.to_string())?;
      fs.write(Path::new("/test_dir/file2.js"), b"content2").map_err(|err| err.to_string())?;
      fs.create_dir_all(Path::new("/test_dir/subdir")).map_err(|err| err.to_string())?;
      fs.write(Path::new("/test_dir/subdir/file3.js"), b"content3")
        .map_err(|err| err.to_string())?;

      // Test reading directory contents.
      let entries = fs.read_dir(Path::new("/test_dir")).map_err(|err| err.to_string())?;

      // Should contain 3 entries: file1.js, file2.js, subdir.
      assert_eq!(entries.len(), 3);

      // Check if expected file paths are present (read_dir returns relative paths).
      let entry_paths: Vec<String> =
        entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
      assert!(entry_paths.contains(&"file1.js".to_string()));
      assert!(entry_paths.contains(&"file2.js".to_string()));
      assert!(entry_paths.contains(&"subdir".to_string()));

      // Test reading non-existent directory should return error.
      let result = fs.read_dir(Path::new("/non_existent_dir"));
      assert!(result.is_err());

      // Test reading file - VFS implementation returns empty vector for files.
      fs.write(Path::new("/test_file.txt"), b"content").map_err(|err| err.to_string())?;
      let result = fs.read_dir(Path::new("/test_file.txt")).map_err(|err| err.to_string())?;
      assert!(result.is_empty());

      Ok(())
    }

    #[test]
    fn empty_directory() -> Result<(), String> {
      let fs = MemoryFileSystem::new(&[]);
      fs.create_dir_all(Path::new("/empty_dir")).map_err(|err| err.to_string())?;

      let entries = fs.read_dir(Path::new("/empty_dir")).map_err(|err| err.to_string())?;
      assert!(entries.is_empty());
      Ok(())
    }

    #[test]
    fn nested_structure() -> Result<(), String> {
      let fs = MemoryFileSystem::new(&[]);
      fs.create_dir_all(Path::new("/root")).map_err(|err| err.to_string())?;
      fs.create_dir_all(Path::new("/root/dir1")).map_err(|err| err.to_string())?;
      fs.create_dir_all(Path::new("/root/dir2")).map_err(|err| err.to_string())?;
      fs.write(Path::new("/root/file1.txt"), b"file1").map_err(|err| err.to_string())?;
      fs.write(Path::new("/root/dir1/file2.txt"), b"file2").map_err(|err| err.to_string())?;
      fs.write(Path::new("/root/dir2/file3.txt"), b"file3").map_err(|err| err.to_string())?;

      let root_entries = fs.read_dir(Path::new("/root")).map_err(|err| err.to_string())?;
      assert_eq!(root_entries.len(), 3); // dir1, dir2, file1.txt

      let root_paths: Vec<String> =
        root_entries.iter().map(|p| p.to_string_lossy().to_string()).collect();
      assert!(root_paths.contains(&"dir1".to_string()));
      assert!(root_paths.contains(&"dir2".to_string()));
      assert!(root_paths.contains(&"file1.txt".to_string()));

      let dir1_entries = fs.read_dir(Path::new("/root/dir1")).map_err(|err| err.to_string())?;
      assert_eq!(dir1_entries.len(), 1);
      assert_eq!(dir1_entries[0].to_string_lossy(), "file2.txt");

      Ok(())
    }
  }
}
