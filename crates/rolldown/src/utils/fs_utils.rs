use std::{io, path::Path};

use rolldown_fs::FileSystem;

/// Empty the contents of a directory without deleting the directory itself.
///
/// 1. When the path is not a directory, it will return `Err`.
/// 2. When the path not exist, nothing will happen, it will return `Ok`.
/// 3. Only when the path is an existing directory, it will empty inside.
/// 4. When the path is a symlink to a directory, only the symlink is removed.
pub fn clean_dir<Fs: FileSystem + ?Sized>(fs: &Fs, path: &Path) -> io::Result<()> {
  // A trailing separator would make symlink_metadata follow the directory symlink.
  let path = path.components().as_path();
  let metadata = match fs.symlink_metadata(path) {
    Ok(metadata) => metadata,
    Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
    Err(err) => return Err(err),
  };

  if metadata.is_symlink() {
    // Rust has no remove_link API, and remove_file cannot remove Windows directory symlinks/junctions.
    // remove_dir_all handles these on both Unix and Windows without following the link.
    return fs.remove_dir_all(path);
  }

  if !metadata.is_dir() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("not a directory: {}", path.display()),
    ));
  }

  // Read all entries in the directory and remove them individually.
  for entry in fs.read_dir(path)? {
    if fs.metadata(&entry)?.is_dir() {
      fs.remove_dir_all(&entry)?;
    } else {
      fs.remove_file(&entry)?;
    }
  }

  Ok(())
}
