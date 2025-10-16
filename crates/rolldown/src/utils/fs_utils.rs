use std::{io, path::Path};

use rolldown_fs::FileSystem;

/// Empty the contents of a directory without deleting the directory itself.
///
/// 1. When the path is not a directory, it will return `Err`.
/// 2. When the path not exist, nothing will happen, it will return `Ok`.
/// 3. Only when the path is an existing directory, it will empty inside.
pub fn clean_dir<Fs: FileSystem + ?Sized>(fs: &Fs, path: &Path) -> io::Result<()> {
  if !fs.exists(path) {
    return Ok(());
  }

  if let Ok(metadata) = fs.metadata(path) {
    if !metadata.is_dir() {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("not a directory: {}", path.display()),
      ));
    }
  }

  // Read all entries in the directory and remove them individually.
  for entry in fs.read_dir(path)? {
    match fs.metadata(&entry)?.is_dir() {
      true => fs.remove_dir_all(&entry)?,
      false => fs.remove_file(&entry)?,
    }
  }

  Ok(())
}
