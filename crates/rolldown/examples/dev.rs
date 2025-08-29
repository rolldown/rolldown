use std::sync::Arc;

use rolldown::dev::DevOptions;
use rolldown::{BundlerBuilder, BundlerOptions, DevEngine, ExperimentalOptions};
use sugar_path::SugarPath;

// RD_LOG=rolldown::dev=trace cargo run --example dev

#[expect(clippy::print_stdout)]
#[tokio::main]
async fn main() {
  let bundler_builder = BundlerBuilder::default().with_options(BundlerOptions {
    input: Some(vec!["./entry.js".to_string().into()]),
    cwd: Some(rolldown_workspace::crate_dir("rolldown").join("./examples/basic").normalize()),

    experimental: Some(ExperimentalOptions { incremental_build: Some(true), ..Default::default() }),
    ..Default::default()
  });
  let dev_engine = DevEngine::<rolldown_watcher::NotifyWatcher>::new(
    bundler_builder,
    DevOptions {
      eager_rebuild: Some(false),
      on_hmr_updates: Some(Arc::new(|updates| {
        println!("HMR updates: {updates:#?}");
      })),
    },
  )
  .unwrap();
  dev_engine.run().await;
  dev_engine.wait_for_close().await;
}
