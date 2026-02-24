use std::sync::Arc;

use rolldown::{BundlerOptions, ExperimentalOptions};
use rolldown_dev::{BundlerConfig, DevEngine, DevOptions, RebuildStrategy};
use sugar_path::SugarPath;

// RD_LOG=rolldown::dev=trace cargo run --example dev

#[expect(clippy::print_stdout)]
#[tokio::main]
async fn main() {
  let bundler_config = BundlerConfig::new(
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
  let dev_engine = DevEngine::new(
    bundler_config,
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
  dev_engine.wait_for_close().await.unwrap();
}
