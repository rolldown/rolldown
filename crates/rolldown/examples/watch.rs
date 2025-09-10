use std::sync::Arc;

use rolldown::{Bundler, BundlerOptions, ExperimentalOptions, Watcher};
use sugar_path::SugarPath;
use tokio::sync::Mutex;

// cargo run --example watch

#[tokio::main]
async fn main() {
  let bundler = Bundler::new(BundlerOptions {
    input: Some(vec!["./entry.js".to_string().into()]),
    cwd: Some(rolldown_workspace::crate_dir("rolldown").join("./examples/basic").normalize()),

    experimental: Some(ExperimentalOptions { incremental_build: Some(true), ..Default::default() }),
    ..Default::default()
  })
  .expect("Failed to create bundler");
  let watcher = Watcher::new(vec![Arc::new(Mutex::new(bundler))], None).unwrap();
  watcher.start().await;
}
