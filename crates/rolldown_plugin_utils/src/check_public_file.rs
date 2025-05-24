use std::path::{Path, PathBuf};

use sugar_path::SugarPath as _;

pub fn check_public_file(id: &str, public_dir: Option<&str>) -> Option<PathBuf> {
  if id.is_empty() || id.as_bytes()[0] != b'/' {
    return None;
  }
  if let Some(dir) = public_dir {
    let file = Path::new(dir).join(&id[1..]).normalize();
    if file.starts_with(dir) && file.exists() {
      return Some(file);
    }
  }
  None
}
