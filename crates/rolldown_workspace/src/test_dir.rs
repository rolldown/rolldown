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

  /// Like [`TestDir::new`], but keeps the canonical path of the directory.
  ///
  /// Module ids are canonical, so a test that matches file-system paths against
  /// module ids needs this: on macOS the temp dir is `/var/...`, a symlink to
  /// `/private/var/...`. `dunce` resolves that while dropping the `\\?\`
  /// verbatim prefix `std::fs::canonicalize` adds on Windows, which the resolver
  /// cannot resolve an entry from.
  ///
  /// # Panics
  ///
  /// Panics if the directory cannot be created or canonicalized.
  pub fn new_canonical(prefix: &str) -> Self {
    let path = create_unique_dir(prefix);
    Self(dunce::canonicalize(&path).expect("canonicalize test directory"))
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

#[cfg(test)]
mod tests {
  use super::TestDir;

  #[test]
  fn each_test_dir_is_unique_and_removed_on_drop() {
    let first = TestDir::new("rolldown-workspace-test-dir");
    let second = TestDir::new("rolldown-workspace-test-dir");
    assert_ne!(first.path(), second.path());
    assert!(first.path().is_dir());

    let path = first.path().to_path_buf();
    drop(first);
    assert!(!path.exists());
  }

  #[test]
  fn canonical_test_dir_is_canonical() {
    let dir = TestDir::new_canonical("rolldown-workspace-test-dir-canonical");
    assert_eq!(dunce::canonicalize(dir.path()).unwrap(), dir.path());
  }
}
