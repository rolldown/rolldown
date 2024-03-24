use rolldown_fs::MemoryFileSystem;
use std::panic;
use std::path::Path;

use wasm_bindgen::prelude::*;

use rolldown::{BundlerBuilder, External, InputItem, InputOptions};
#[wasm_bindgen]
pub struct FileItem {
  path: String,
  content: String,
  is_entry: bool,
}

#[wasm_bindgen]
impl FileItem {
  #[wasm_bindgen(constructor)]
  pub fn new(path: String, content: String, is_entry: bool) -> Self {
    Self { path, content, is_entry }
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
      let _memory_fs = MemoryFileSystem::new(
        &file_list.iter().map(|item| (&item.path, &item.content)).collect::<Vec<_>>(),
      );
      let input = file_list
        .into_iter()
        .filter_map(|item| {
          if item.is_entry {
            let p = Path::new(&item.path);
            let name = p.file_stem().map(|stem| stem.to_string_lossy().replace('.', "_"));
            Some(InputItem { name, import: item.path })
          } else {
            None
          }
        })
        .collect::<Vec<_>>();
      let mut bundler = BundlerBuilder::default()
        .with_input_options(InputOptions {
          input,
          cwd: Some("/".into()),
          external: Some(External::ArrayString(vec![])),
          treeshake: Some(false),
          resolve: None,
        })
        .build();

      match bundler.write().await {
        Ok(assets) => assets
          .assets
          .into_iter()
          .map(|item| AssetItem {
            name: item.file_name().to_string(),
            content: item.content().to_owned(),
          })
          .collect::<Vec<_>>(),
        Err(err) => {
          panic!("{err:?}",);
        }
      }
    });
  result
}
