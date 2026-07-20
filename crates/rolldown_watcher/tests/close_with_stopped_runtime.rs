//! `Watcher::close()` submits the retained coordinator through the shared async
//! runtime, so a stopped runtime rejects the close the same way it rejects
//! `run()`. This locks in that the rejection is *retryable*: the coordinator --
//! and with it every fs watcher and bundler -- stays retained instead of being
//! silently dropped, and a later `close()` on a restarted runtime still runs
//! the close hooks, closes the bundlers, and releases everything.
//!
//! The JavaScript wrapper in `packages/rolldown/src/api/watch/watcher.ts`
//! depends on exactly this contract: it must not latch its `closed` flag on a
//! rejected close, or this recovery path becomes unreachable.
//!
//! This lives in its own integration-test binary because it stops the
//! process-global async runtime.

use rolldown::{BundlerConfig, BundlerOptions};
use rolldown_common::WatcherChangeKind;
use rolldown_utils::async_runtime;
use rolldown_watcher::{WatchEvent, Watcher, WatcherConfig, WatcherEventHandler};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Stands in for everything the coordinator owns (fs watchers, bundlers, the
/// event handler). `Drop` fires only when the retained coordinator future is
/// released, so it observes whether close actually tore the watcher down.
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

struct TestDir(PathBuf);

impl TestDir {
  fn new() -> Self {
    let path = std::env::temp_dir().join(format!(
      "rolldown-watcher-stopped-runtime-{}",
      std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("create test directory");
    Self(path)
  }
}

impl Drop for TestDir {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

#[test]
fn close_rejected_by_a_stopped_runtime_stays_retryable() {
  let dir = TestDir::new();
  let input = dir.0.join("main.js");
  std::fs::write(&input, "export const value = 1;\n").expect("write input");

  let dropped = Arc::new(AtomicBool::new(false));
  let closes = Arc::new(AtomicUsize::new(0));
  let config = BundlerConfig::new(
    BundlerOptions {
      cwd: Some(dir.0.clone()),
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

  // The watcher was never `run()`, so the coordinator -- holding the fs
  // watchers and bundlers -- is still retained for a later submission.
  async_runtime::shutdown().expect("stop the shared async runtime");

  let rejected = futures::executor::block_on(watcher.close())
    .expect_err("a stopped runtime must reject the close submission");
  assert!(
    rejected.to_string().starts_with("Watcher coordinator task submission failed:"),
    "unexpected close error: {rejected}"
  );
  // The rejection must not fake a teardown: nothing was closed or released.
  assert_eq!(closes.load(Ordering::SeqCst), 0, "close hooks must not run for a rejected close");
  assert!(!dropped.load(Ordering::SeqCst), "a rejected close must retain the coordinator");

  // Restarting the runtime makes the retained coordinator submittable again,
  // and the retry performs the real teardown.
  async_runtime::start().expect("restart the shared async runtime");
  futures::executor::block_on(watcher.close())
    .expect("a restarted runtime must accept the retried close");
  assert_eq!(closes.load(Ordering::SeqCst), 1, "the retried close must run the close hooks");
  assert!(dropped.load(Ordering::SeqCst), "the retried close must release the coordinator");

  // Still idempotent once it has succeeded.
  futures::executor::block_on(watcher.close()).expect("a completed close must stay idempotent");
  assert_eq!(closes.load(Ordering::SeqCst), 1, "close must not re-run its hooks");
}
