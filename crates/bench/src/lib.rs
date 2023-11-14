use std::path::PathBuf;

use rolldown::{Bundler, InputItem, InputOptions, OutputOptions};

pub fn repo_root() -> PathBuf {
  let project_root = PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").unwrap());
  project_root.parent().unwrap().parent().unwrap().to_path_buf()
}

pub async fn run_fixture(fixture_path: PathBuf) {
  let mut bundler = Bundler::new(InputOptions {
    input: vec![InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }],
    cwd: fixture_path.clone(),
    ..Default::default()
  });

  if fixture_path.join("dist").is_dir() {
    std::fs::remove_dir_all(fixture_path.join("dist")).unwrap();
  }

  bundler.write(OutputOptions::default()).await.unwrap();
}

pub fn join_by_repo_root(path: &str) -> PathBuf {
  repo_root().join(path)
}
