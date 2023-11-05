use std::path::PathBuf;
use wasm_bindgen::prelude::*;

use rolldown::{Bundler, InputItem, InputOptions};
use sugar_path::SugarPathBuf;

#[tokio::main(flavor = "current_thread")]
#[wasm_bindgen]
pub async fn greet(root: &str) {
  let root = PathBuf::from(root);
  let cwd = root.join("./examples").into_normalize();
  let mut bundler = Bundler::new(InputOptions {
    input: Some(vec![InputItem {
      name: Some("basic".to_string()),
      import: "./index.js".to_string(),
    }]),
    cwd: Some(cwd),
  });

  let outputs = bundler.write(Default::default()).await.unwrap();
  println!("{outputs:#?}");
}
