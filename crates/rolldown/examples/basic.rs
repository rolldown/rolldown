use std::path::PathBuf;

use rolldown::{Bundler, InputOptions};
use sugar_path::SugarPathBuf;

#[tokio::main]
async fn main() {
  let root = PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").unwrap());
  let cwd = root.join("./examples").into_normalize();
  let mut bundler = Bundler::new(InputOptions {
    input: Some(vec!["./index.js".to_string().into()]),
    cwd: Some(cwd),
  });

  bundler.generate(Default::default()).await.unwrap();
}
