use std::path::Path;

use vfs::VfsPath;

use crate::FileSystem;

pub struct FileSystemVfs {
  // root path
  inner: VfsPath,
}

impl FileSystem for FileSystemVfs {
  fn read_to_string(&self, path: &Path) -> anyhow::Result<String> {
    let cur_path = self.inner.join(path.to_string_lossy())?;
    let mut buf = String::new();
    cur_path.open_file()?.read_to_string(&mut buf)?;
    Ok(buf)
  }
}
