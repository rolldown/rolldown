use rolldown_fs::{FileSystemVfs};
use std::panic;

use wasm_bindgen::prelude::*;

use rolldown::{Bundler, InputItem, InputOptions, OutputOptions};


#[wasm_bindgen]
pub fn greet(_name: &str) -> String {
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
      
      match bundler.write(OutputOptions::default()).await {
        Ok(data) => {
          format!("{data:?}")
        }
        Err(err) => {
          format!("{err:?}")
        }
      }
    });
  res
}
