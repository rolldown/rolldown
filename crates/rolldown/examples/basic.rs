use std::collections::HashMap;

use rolldown::{Bundler, BundlerOptions, InputItem, ModuleType, SourceMapType};
use rolldown_testing::workspace;
use sugar_path::SugarPath;

// cargo run --example basic

#[tokio::main]
async fn main() {
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![
      "./entry.js".to_string().into(),
      // InputItem { import: "./other-entry.js".to_string(), ..Default::default() },
      // InputItem { name: Some("third-entry".to_string()), import: "./third-entry.js".to_string() },
    ]),
    module_types: Some(HashMap::from_iter([(".custom".to_string(), ModuleType::Dataurl)])),
    cwd: Some(workspace::crate_dir("rolldown").join("./examples/basic").normalize()),
    sourcemap: Some(SourceMapType::File),
    ..Default::default()
  });

  let result = bundler.write().await.unwrap();
  assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
}
