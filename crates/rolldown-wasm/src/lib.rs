use rolldown_fs::{FileSystemOs, FileSystemVfs};
use std::path::PathBuf;
use wasm_bindgen::prelude::*;

use rolldown::{Bundler, InputItem, InputOptions};
use sugar_path::SugarPathBuf;

#[tokio::main(flavor = "current_thread")]
#[wasm_bindgen]
pub async fn greet(name: &str) {
  let memory_fs = FileSystemVfs::new(&[("/index.js", "const a = 3000000")]);
  let mut bundler = Bundler::new(
    InputOptions {
      input: Some(vec![InputItem {
        name: Some("basic".to_string()),
        import: "./index.js".to_string(),
      }]),
      cwd: Some("/".into()),
    },
    memory_fs,
  );

  let outputs = bundler.write(Default::default()).await.unwrap();
  println!("{outputs:#?}");
}
