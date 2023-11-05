use std::path::Path;

use vfs::VfsPath;

use crate::FileSystem;

pub struct FileSystemVfs {
  // root path
  root: VfsPath,
}

impl FileSystem for FileSystemVfs {
  fn read_to_string(&self, path: &Path) -> anyhow::Result<String> {
    let cur_path = self.root.join(path.to_string_lossy())?;
    let mut buf = String::new();
    cur_path.open_file()?.read_to_string(&mut buf)?;
    Ok(buf)
  }

  fn remove_dir_all(&self, path: &Path) -> anyhow::Result<()> {
    let cur_path = self.root.join(path.to_string_lossy())?;
    cur_path.remove_dir_all()?;
    Ok(())
  }

  fn create_dir_all(&self, path: &Path) -> anyhow::Result<()> {
    let cur_path = self.root.join(path.to_string_lossy())?;
    cur_path.create_dir_all()?;
    Ok(())
  }

  fn write(&self, path: &Path, content: &[u8]) -> anyhow::Result<()> {
    let cur_path = self.root.join(path.to_string_lossy())?;
    cur_path.create_file()?.write(content)?;
    Ok(())
  }
}
