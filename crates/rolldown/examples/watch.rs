use rolldown::{BundlerConfig, BundlerOptions, ExperimentalOptions, Watcher};
use sugar_path::SugarPath;

// cargo run --example watch

#[tokio::main]
async fn main() {
  let config = BundlerConfig::new(
    BundlerOptions {
      input: Some(vec!["./entry.js".to_string().into()]),
      cwd: Some(
        rolldown_workspace::crate_dir("rolldown").join("./examples/basic").normalize().into_owned(),
      ),

      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![],
  );
  let watcher = Watcher::new(config).unwrap();
  watcher.start().await;
}
