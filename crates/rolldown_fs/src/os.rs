use crate::FileSystem;
use std::{fs, path::Path};

/// Operating System
pub struct FileSystemOs;

impl FileSystem for FileSystemOs {
  fn read_to_string(&self, path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).map_err(|err| anyhow::anyhow!(err))
  }

  fn remove_dir_all(&self, path: &Path) -> anyhow::Result<()> {
    std::fs::remove_dir_all(path).map_err(|err| anyhow::anyhow!(err))
  }

  fn create_dir_all(&self, path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path).map_err(|err| anyhow::anyhow!(err))
  }

  fn write(&self, path: &Path, content: &[u8]) -> anyhow::Result<()> {
    std::fs::write(path, content).map_err(|err| anyhow::anyhow!(err))
  }
}
