use std::path::PathBuf;

use rolldown::{Bundler, InputItem, InputOptions, OutputOptions};

pub async fn run_fixture(fixture_path: PathBuf) {
  let mut bundler = Bundler::new(
    InputOptions {
      input: vec![InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }],
      cwd: fixture_path.clone(),
      ..Default::default()
    },
    FileSystemOs,
  );

  if fixture_path.join("dist").is_dir() {
    std::fs::remove_dir_all(fixture_path.join("dist")).unwrap();
  }

  bundler.write(OutputOptions::default()).await.unwrap();
}

pub fn normalized_fixture_path(path: &str) -> PathBuf {
  let project_root = std::env::current_dir().unwrap();
  project_root.join(path)
}
