//! A stopped async runtime makes `Watcher::close()` fail instead of hang: the
//! close starts the never-run coordinator, the runtime refuses it, and the
//! refused coordinator -- with every fs watcher and bundler it owns -- is
//! released with the error.
//!
//! Own test binary: it stops the process-global async runtime.

use rolldown::{BundlerConfig, BundlerOptions};
use rolldown_common::WatcherChangeKind;
use rolldown_utils::async_runtime;
use rolldown_watcher::{WatchEvent, Watcher, WatcherConfig, WatcherEventHandler};
use rolldown_workspace::TestDir;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Stands in for everything the coordinator owns. `Drop` fires only when the
/// coordinator future is released, so it observes a real teardown.
struct Probe {
  dropped: Arc<AtomicBool>,
  closes: Arc<AtomicUsize>,
}

impl Drop for Probe {
  fn drop(&mut self) {
    self.dropped.store(true, Ordering::SeqCst);
  }
}

impl WatcherEventHandler for Probe {
  async fn on_event(&self, _event: WatchEvent) {}
  async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {}
  async fn on_restart(&self) {}
  async fn on_close(&self) {
    self.closes.fetch_add(1, Ordering::SeqCst);
  }
}

#[test]
fn close_on_a_stopped_runtime_fails_without_hanging() {
  let dir = TestDir::new("rolldown-watcher-stopped-runtime");
  let input = dir.path().join("main.js");
  std::fs::write(&input, "export const value = 1;\n").expect("write input");

  let dropped = Arc::new(AtomicBool::new(false));
  let closes = Arc::new(AtomicUsize::new(0));
  let config = BundlerConfig::new(
    BundlerOptions {
      cwd: Some(dir.path().to_path_buf()),
      input: Some(vec![input.to_string_lossy().into_owned().into()]),
      ..Default::default()
    },
    vec![],
  );
  let watcher = Watcher::new(
    vec![config],
    Probe { dropped: Arc::clone(&dropped), closes: Arc::clone(&closes) },
    &WatcherConfig::default(),
  )
  .unwrap_or_else(|errors| panic!("create watcher: {errors:?}"));

  async_runtime::shutdown().expect("stop the shared async runtime");

  let rejected = futures::executor::block_on(watcher.close())
    .expect_err("a stopped runtime must reject the close submission");
  assert!(
    rejected.to_string().starts_with("Watcher coordinator task submission failed:"),
    "unexpected close error: {rejected}"
  );
  assert_eq!(closes.load(Ordering::SeqCst), 0, "close hooks must not run for a refused close");
  assert!(dropped.load(Ordering::SeqCst), "a refused close must release the coordinator");

  // Nothing is left to start or await, so a later close resolves at once.
  async_runtime::start().expect("restart the shared async runtime");
  futures::executor::block_on(watcher.close()).expect("a later close must resolve");
}
