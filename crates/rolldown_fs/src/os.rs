use crate::FileSystem;
use std::{
  fs,
  path::{Path, PathBuf},
};

/// Operating System
pub struct FileSystemOs;

impl FileSystem for FileSystemOs {
  fn read_to_string(&self, path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).map_err(|err| anyhow::anyhow!(err))
  }
}
