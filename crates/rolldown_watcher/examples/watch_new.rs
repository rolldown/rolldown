#![expect(clippy::print_stdout, reason = "Example uses stdout for demonstration")]

use rolldown::{BundlerConfig, BundlerOptions, ExperimentalOptions};
use rolldown_watcher::{Watcher, WatcherConfig, WatcherEvent};
use sugar_path::SugarPath;

// cargo run -p rolldown_watcher --example watch_new

#[tokio::main]
async fn main() {
  let config = BundlerConfig::new(
    BundlerOptions {
      input: Some(vec!["./entry.js".to_string().into()]),
      cwd: Some(rolldown_workspace::crate_dir("rolldown").join("./examples/basic").normalize()),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![],
  );

  let watcher = Watcher::new(config, &WatcherConfig::default()).expect("Failed to create watcher");

  // Subscribe to events
  let mut events = watcher.emitter().subscribe();
  let event_handle = tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
      match &event {
        WatcherEvent::Event(bundle_event) => {
          println!("[Event] {bundle_event}");
        }
        WatcherEvent::Change(data) => {
          println!("[Change] {:?}: {}", data.kind, data.path);
        }
        WatcherEvent::Close => {
          println!("[Close] Watcher closed");
          break;
        }
      }
    }
  });

  println!("Watching for changes... Press Ctrl+C to stop.");

  // Wait for Ctrl+C
  tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");

  println!("\nClosing watcher...");
  watcher.close().await.expect("Failed to close watcher");

  // Wait for event handler to finish
  let _ = event_handle.await;

  println!("Done.");
}
