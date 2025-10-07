use std::sync::Arc;

use rolldown::dev::{DevOptions, RebuildStrategy};
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
  let dev_engine = DevEngine::new(
    bundler_builder,
    DevOptions {
      rebuild_strategy: Some(RebuildStrategy::Always),
      on_hmr_updates: Some(Arc::new(|result| match result {
        Ok((updates, changed_files)) => {
          println!("HMR updates: {updates:#?} due to {changed_files:#?}");
        }
        Err(e) => {
          eprintln!("HMR error: {e:#?}");
        }
      })),
      ..Default::default()
    },
  )
  .unwrap();
  dev_engine.run().await.unwrap();
  dev_engine.wait_for_build_driver_service_close().await.unwrap();
}
