#![expect(clippy::print_stdout, reason = "Example uses stdout for demonstration")]

use rolldown::{BundlerConfig, BundlerOptions, ExperimentalOptions};
use rolldown_common::WatcherChangeKind;
use rolldown_watcher::{WatchEvent, Watcher, WatcherConfig, WatcherEventHandler};
use sugar_path::SugarPath;

// cargo run -p rolldown_watcher --example watch

struct PrintHandler;

impl WatcherEventHandler for PrintHandler {
  async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
    println!("[Event] {event}");
    Ok(())
  }

  async fn on_change(&self, path: &str, kind: WatcherChangeKind) -> anyhow::Result<()> {
    println!("[Change] {kind:?}: {path}");
    Ok(())
  }

  async fn on_restart(&self) -> anyhow::Result<()> {
    println!("[Restart]");
    Ok(())
  }

  async fn on_close(&self) -> anyhow::Result<()> {
    println!("[Close] Watcher closed");
    Ok(())
  }
}

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

  let watcher = Watcher::new(vec![config], PrintHandler, &WatcherConfig::default())
    .expect("Failed to create watcher");
  watcher.run();

  println!("Watching for changes... Press Ctrl+C to stop.");

  // Keep the watcher alive; in real usage you'd use tokio::signal::ctrl_c()
  // but that requires the "signal" tokio feature which isn't enabled for tests.
  tokio::time::sleep(std::time::Duration::from_secs(10)).await;

  println!("\nClosing watcher...");
  watcher.close().await.expect("Failed to close watcher");

  println!("Done.");
}
