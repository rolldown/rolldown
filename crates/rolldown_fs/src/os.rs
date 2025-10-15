use std::{
  fmt, io,
  path::{Path, PathBuf},
  sync::Arc,
};

use oxc_resolver::{FileMetadata, FileSystem as OxcResolverFileSystem, FileSystemOs, ResolveError};

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

  fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(path)?;
    let mut paths = Vec::new();
    for entry in entries {
      let entry = entry?;
      paths.push(entry.path());
    }
    Ok(paths)
  }

  fn remove_file(&self, path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)
  }
}

impl crate::file_system::FileSystemUtils for OsFileSystem {}

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

  fn read_link(&self, path: &Path) -> Result<PathBuf, ResolveError> {
    self.0.read_link(path)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::file_system::FileSystemUtils;
  use std::env;

  #[cfg(test)]
  mod clean_dir {
    use super::*;

    #[test]
    fn files_and_sub_dirs() -> io::Result<()> {
      let fs = OsFileSystem::new(false);

      // Create a temporary directory for testing.
      let temp_dir = env::temp_dir().join("rolldown_fs_test_clean_dir");
      if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
      }
      std::fs::create_dir_all(&temp_dir)?;

      // Create directory structure.
      let test_dir = temp_dir.join("test_dir");
      std::fs::create_dir_all(&test_dir)?;
      std::fs::write(test_dir.join("file1.txt"), b"content1")?;
      std::fs::write(test_dir.join("file2.txt"), b"content2")?;
      let subdir = test_dir.join("subdir");
      std::fs::create_dir_all(&subdir)?;
      std::fs::write(subdir.join("file3.txt"), b"content3")?;

      // Verify files exist before cleaning.
      assert!(test_dir.exists());
      assert!(test_dir.join("file1.txt").exists());
      assert!(test_dir.join("file2.txt").exists());
      assert!(subdir.exists());
      assert!(subdir.join("file3.txt").exists());

      fs.clean_dir(&test_dir)?;
      assert!(test_dir.exists());
      assert!(!test_dir.join("file1.txt").exists());
      assert!(!test_dir.join("file2.txt").exists());
      assert!(!subdir.exists());

      std::fs::remove_dir_all(&temp_dir)?;
      Ok(())
    }

    #[test]
    fn non_existent_dir() -> io::Result<()> {
      let fs = OsFileSystem::new(false);
      let temp_dir = env::temp_dir().join("rolldown_fs_test_nonexistent");
      fs.clean_dir(&temp_dir.join("non_existent"))?;

      Ok(())
    }

    #[test]
    fn clean_file_should_fail() -> io::Result<()> {
      let fs = OsFileSystem::new(false);
      let temp_dir = env::temp_dir().join("rolldown_fs_test_file");
      if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
      }
      std::fs::create_dir_all(&temp_dir)?;

      let test_file = temp_dir.join("test_file.txt");
      std::fs::write(&test_file, b"content")?;
      let result = fs.clean_dir(&test_file);
      assert!(result.is_err());

      std::fs::remove_dir_all(&temp_dir)?;
      Ok(())
    }

    #[test]
    fn empty_dir() -> io::Result<()> {
      let fs = OsFileSystem::new(false);
      let temp_dir = env::temp_dir().join("rolldown_fs_test_empty");
      if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
      }
      std::fs::create_dir_all(&temp_dir)?;

      let empty_dir = temp_dir.join("empty_dir");
      std::fs::create_dir_all(&empty_dir)?;
      assert!(empty_dir.exists());

      fs.clean_dir(&empty_dir)?;
      assert!(empty_dir.exists());

      std::fs::remove_dir_all(&temp_dir)?;
      Ok(())
    }
  }
}
