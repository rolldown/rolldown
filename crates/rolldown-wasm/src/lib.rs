use rolldown_fs::{FileSystemOs, FileSystemVfs};
use std::panic;
use std::path::PathBuf;
use wasm_bindgen::prelude::*;

use rolldown::{Bundler, InputItem, InputOptions};
use sugar_path::SugarPathBuf;

#[wasm_bindgen]
pub async fn greet(name: &str) -> String {
  panic::set_hook(Box::new(console_error_panic_hook::hook));
  let res =
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
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
      //
      let output = match bundler.write(Default::default()).await {
        Ok(data) => {
          format!("{:?}", data)
        }
        Err(err) => {
          format!("{:?}", err)
        }
      };
      output
    });
  res
}
