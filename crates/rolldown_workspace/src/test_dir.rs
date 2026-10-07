use std::{
  path::{Path, PathBuf},
  sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

/// A temporary directory for one test, removed (best effort) when dropped.
///
/// The directory is `std::env::temp_dir()/{prefix}-{pid}-{n}`, where `n` counts up
/// once per `TestDir` in this process, so tests running in parallel never share
/// a directory.
#[derive(Debug)]
pub struct TestDir(PathBuf);

impl TestDir {
  /// Creates `{prefix}-{pid}-{n}` under the system temp directory.
  ///
  /// # Panics
  ///
  /// Panics if the directory cannot be created.
  pub fn new(prefix: &str) -> Self {
    Self(create_unique_dir(prefix))
  }

  pub fn path(&self) -> &Path {
    &self.0
  }
}

impl Drop for TestDir {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

fn create_unique_dir(prefix: &str) -> PathBuf {
  let path = std::env::temp_dir().join(format!(
    "{prefix}-{}-{}",
    std::process::id(),
    NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
  ));
  std::fs::create_dir_all(&path).expect("create test directory");
  path
}
