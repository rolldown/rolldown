use rolldown_fs::FileSystemVfs;
use std::panic;

use wasm_bindgen::prelude::*;

use rolldown::{Bundler, InputItem, InputOptions, OutputOptions};
#[wasm_bindgen]
pub struct FileItem {
  path: String,
  content: String,
}

#[wasm_bindgen]
impl FileItem {
  #[wasm_bindgen(constructor)]
  pub fn new(path: String, content: String) -> Self {
    Self { path, content }
  }
}

#[wasm_bindgen]
pub struct AssetItem {
  name: String,
  content: String,
}

#[wasm_bindgen]
impl AssetItem {
  #[wasm_bindgen(getter)]
  pub fn name(&self) -> String {
    self.name.clone()
  }

  #[wasm_bindgen(getter)]
  pub fn content(&self) -> String {
    self.content.clone()
  }
}

#[allow(clippy::needless_pass_by_value)]
#[wasm_bindgen]
pub fn bundle(file_list: Vec<FileItem>) -> Vec<AssetItem> {
  panic::set_hook(Box::new(console_error_panic_hook::hook));
  let result =
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
      let memory_fs = FileSystemVfs::new(
        &file_list.iter().map(|item| (&item.path, &item.content)).collect::<Vec<_>>(),
      );
      let mut bundler = Bundler::new(
        InputOptions {
          input: vec![InputItem {
            name: Some("basic".to_string()),
            import: "./index.js".to_string(),
          }],
          cwd: "/".into(),
        },
        memory_fs,
      );

      match bundler.write(OutputOptions::default()).await {
        Ok(assets) => assets
          .into_iter()
          .map(|item| AssetItem { name: item.file_name, content: item.code })
          .collect::<Vec<_>>(),
        Err(err) => {
          panic!("{err:?}",);
        }
      }
    });
  result
}
