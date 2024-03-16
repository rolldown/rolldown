use std::path::PathBuf;

use rolldown::{Bundler, InputItem, InputOptions, OutputOptions};

pub fn repo_root() -> PathBuf {
  let cargo_manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map(PathBuf::from);
  let project_root = if let Ok(cargo_manifest_dir) = cargo_manifest_dir {
    cargo_manifest_dir.parent().unwrap().parent().unwrap().to_path_buf()
  } else {
    std::env::current_dir().expect("failed to get current dir")
  };

  assert_eq!(
    project_root.file_name().unwrap(),
    "rolldown",
    "Benchmark must be run from the root of the repo, got wrong `project_root` {}",
    project_root.display()
  );
  project_root
}

pub async fn run_fixture(fixture_path: PathBuf) {
  let mut bundler = Bundler::new(
    InputOptions {
      input: vec![InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }],
      cwd: fixture_path.clone(),
      ..Default::default()
    },
    OutputOptions::default(),
  );

  if fixture_path.join("dist").is_dir() {
    std::fs::remove_dir_all(fixture_path.join("dist")).unwrap();
  }

  bundler.write().await.unwrap();
}

pub fn join_by_repo_root(path: &str) -> PathBuf {
  repo_root().join(path)
}
