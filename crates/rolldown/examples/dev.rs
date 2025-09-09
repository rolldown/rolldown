use std::sync::Arc;

use rolldown::dev::DevOptions;
use rolldown::{BundlerBuilder, BundlerOptions, DevEngine, ExperimentalOptions};
use sugar_path::SugarPath;

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
      eager_rebuild: Some(false),
      on_hmr_updates: Some(Arc::new(|updates, _changed_files| {
        println!("HMR updates: {updates:#?}");
      })),
      ..Default::default()
    },
  )
  .unwrap();

  println!("Starting DevEngine...");
  dev_engine.run().await.unwrap();

  // Demonstrate the close method: run for 10 seconds then close
  println!("DevEngine running, will close automatically after 10 seconds...");

  // Wait for 10 seconds
  tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

  println!("10 seconds elapsed, closing DevEngine...");

  // Call the close method to clean up resources
  dev_engine.close().await.unwrap();

  println!("DevEngine closed successfully!");
}
