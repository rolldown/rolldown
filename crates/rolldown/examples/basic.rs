use std::{path::PathBuf, sync::Arc};

use rolldown::{Bundler, InputItem, InputOptions};
use rolldown_fs::{FileSystemOs, FileSystemVfs};
use sugar_path::SugarPathBuf;

#[tokio::main]
async fn main() {
  // let root = PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").unwrap());
  // let cwd = root.join("./examples").into_normalize();
  let fs = FileSystemVfs::new(&[("/index.js", "const a = 3000000")]);
  let mut bundler = Bundler::new(
    InputOptions {
      input: Some(vec![InputItem {
        name: Some("basic".to_string()),
        import: "./index.js".to_string(),
      }]),
      cwd: Some("/".into()),
    },
    fs,
  );

  let outputs = bundler.write(Default::default()).await.unwrap();
}
