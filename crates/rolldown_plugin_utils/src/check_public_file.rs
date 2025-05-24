use std::path::{Path, PathBuf};

use rolldown_utils::url::clean_url;
use sugar_path::SugarPath as _;

pub fn check_public_file(id: &str, public_dir: &str) -> Option<PathBuf> {
  if public_dir.is_empty() || !id.starts_with('/') {
    return None;
  }
  let id = clean_url(id);
  let file = Path::new(public_dir).join(&id[1..]).normalize();
  (file.starts_with(public_dir) && file.exists()).then_some(file)
}
