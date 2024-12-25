use std::sync::Arc;

use rolldown::{Bundler, BundlerOptions, SourceMapType, Watcher};
use rolldown_testing::workspace;
use sugar_path::SugarPath;
use tokio::sync::Mutex;

// cargo run --example watch

#[tokio::main]
async fn main() {
  let bundler = Bundler::new(BundlerOptions {
    input: Some(vec!["./entry.js".to_string().into()]),
    cwd: Some(workspace::crate_dir("rolldown").join("./examples/basic").normalize()),
    sourcemap: Some(SourceMapType::File),
    ..Default::default()
  });
  let watcher = Watcher::new(vec![Arc::new(Mutex::new(bundler))], None).unwrap();
  watcher.start().await;
}
