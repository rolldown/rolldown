use std::path::PathBuf;

use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_testing::workspace;

pub async fn run_fixture(fixture_path: PathBuf) {
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("main".to_string()),
      import: "./main.js".to_string(),
    }]),
    cwd: fixture_path.clone().into(),
    ..Default::default()
  });

  if fixture_path.join("dist").is_dir() {
    std::fs::remove_dir_all(fixture_path.join("dist")).unwrap();
  }

  let result = bundler.write().await.unwrap();
  assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
}

pub fn join_by_workspace_root(path: &str) -> PathBuf {
  workspace::root_dir().join(path)
}
