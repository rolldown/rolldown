use rolldown::{Bundler, BundlerOptions, DevModeOptions, ExperimentalOptions};
use rolldown_workspace as workspace;
use sugar_path::SugarPath as _;

// cargo run --example lazy

#[tokio::main]
async fn main() {
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec!["./entry-a.js".to_string().into(), "./entry-b.js".to_string().into()]),
    cwd: Some(workspace::crate_dir("rolldown").join("./examples/lazy").normalize()),
    sourcemap: None,
    experimental: Some(ExperimentalOptions {
      dev_mode: Some(DevModeOptions { lazy: Some(true), ..Default::default() }),
      ..Default::default()
    }),
    ..Default::default()
  })
  .expect("Failed to create bundler");

  let _result = bundler.write().await.unwrap();
}
