use std::path::{Path, PathBuf};

use rolldown_utils::url::clean_url;
use sugar_path::SugarPath as _;

/// Check if the given URL path corresponds to a file in the public directory.
pub fn check_public_file(path: &str, public_dir: &str) -> Option<PathBuf> {
  if public_dir.is_empty() || !path.starts_with('/') {
    return None;
  }
  let path = &clean_url(path)[1..];
  let file = Path::new(public_dir).join(path).normalize();
  (file.starts_with(public_dir) && file.exists()).then_some(file)
}
