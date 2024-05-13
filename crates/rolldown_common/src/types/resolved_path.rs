use std::{path::Path, sync::Arc};

use super::resource_id::stabilize_resource_id;

#[derive(Debug, Clone)]
pub struct ResolvedPath {
  pub path: Arc<str>,
  pub ignored: bool,
}

impl From<String> for ResolvedPath {
  fn from(value: String) -> Self {
    Self { path: value.into(), ignored: false }
  }
}

impl ResolvedPath {
  /// Created a pretty string representation of the path. The path
  /// 1. doesn't guarantee to be unique
  /// 2. relative to the cwd, so it could show stable path across different machines
  pub fn debug_display(&self, cwd: impl AsRef<Path>) -> String {
    let stable = stabilize_resource_id(&self.path, cwd.as_ref());
    if self.ignored {
      format!("(ignored) {stable}")
    } else {
      stable
    }
  }
}

#[test]
fn test() {
  let mut current_dir = std::env::current_dir().unwrap().display().to_string();
  current_dir.push_str("/resolved_path.rs");
  let mut from_test = ResolvedPath::from(current_dir);

  let prettify_res = from_test.debug_display(Path::new("../"));

  assert_eq!(prettify_res, "rolldown_common/resolved_path.rs");

  from_test.ignored = true;

  let ignore_prettify_res = from_test.debug_display(Path::new("./"));

  assert_eq!(ignore_prettify_res, "(ignored) resolved_path.rs");
}
