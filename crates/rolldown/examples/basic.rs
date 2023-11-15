use std::path::PathBuf;

use rolldown::{Bundler, InputItem, InputOptions};
use sugar_path::SugarPathBuf;

#[tokio::main]
async fn main() {
  let root = PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").unwrap());
  let repo_root = root.parent().unwrap().parent().unwrap();
  let cwd = root.join("./examples").into_normalize();
  let mut bundler = Bundler::new(InputOptions {
    input: vec![InputItem {
      name: Some("basic".to_string()),
      import: repo_root
        .join("temp/three10x/entry.js")
        .into_normalize()
        .to_string_lossy()
        .to_string(),
    }],
    cwd,
    ..Default::default()
  });

  let outputs = bundler.write(Default::default()).await.unwrap();
  println!("{outputs:#?}");
}
