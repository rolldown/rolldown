use std::sync::Arc;

use rolldown::{Bundler, BundlerOptions, ExperimentalOptions, InputItem, ResolveOptions, Watcher};
use rolldown_testing::workspace::{crate_dir, root_dir};
use tokio::sync::Mutex;

#[cfg(not(target_family = "wasm"))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

// cargo run --example watch

#[tokio::main]
async fn main() {
  let bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("rome-ts".to_string()),
      import: "src/index.jsx".into(),
    }]),
    cwd: Some("/home/victor/Documents/rolldown-rs/performance-compare-ext/apps/10000".into()),

    // --- Required specific options for Rome
    // shim_missing_exports: Some(true), // Need this due rome is not written with `isolatedModules: true`
    // resolve: Some(ResolveOptions {
    //   tsconfig_filename: Some(
    //     root_dir().join("tmp/bench/rome/src/tsconfig.json").to_str().unwrap().to_string(),
    //   ),
    //   ..Default::default()
    // }),
    experimental: Some(ExperimentalOptions { incremental_build: Some(true), ..Default::default() }),
    ..Default::default()
  });
  let watcher = Watcher::new(vec![Arc::new(Mutex::new(bundler))], None).unwrap();
  watcher.start().await;
}
