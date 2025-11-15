use rolldown::{BundlerOptions, ExperimentalOptions, Watcher};
use sugar_path::SugarPath;

// cargo run --example watch

#[tokio::main]
async fn main() {
  let bundler_options = BundlerOptions {
    input: Some(vec!["./entry.js".to_string().into()]),
    cwd: Some(rolldown_workspace::crate_dir("rolldown").join("./examples/basic").normalize()),

    experimental: Some(ExperimentalOptions { incremental_build: Some(true), ..Default::default() }),
    ..Default::default()
  };

  let watcher =
    Watcher::new(vec![(bundler_options, vec![])], None).expect("Failed to create watcher");
  watcher.start().await;
}
