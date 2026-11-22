//! A stopped async runtime makes `Watcher::run()` / `Watcher::close()` fail
//! instead of hang, while preserving the coordinator so a later `run()` or
//! `close()` after restart can still run or emit the `close` event, and
//! dropping the watcher releases everything it owns.
//!
//! Own test binary: it stops the process-global async runtime.

use event_listener::Event;
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
  bundle_ends: Arc<AtomicUsize>,
  bundle_end_notify: Arc<Event>,
  closes: Arc<AtomicUsize>,
}

impl Drop for Probe {
  fn drop(&mut self) {
    self.dropped.store(true, Ordering::SeqCst);
  }
}

impl WatcherEventHandler for Probe {
  async fn on_event(&self, event: WatchEvent) {
    if matches!(event, WatchEvent::BundleEnd(_)) {
      self.bundle_ends.fetch_add(1, Ordering::SeqCst);
      self.bundle_end_notify.notify(usize::MAX);
    }
  }
  async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {}
  async fn on_restart(&self) {}
  async fn on_close(&self) {
    self.closes.fetch_add(1, Ordering::SeqCst);
  }
}

fn create_probe_watcher(
  name: &str,
) -> (TestDir, Watcher, Arc<AtomicBool>, Arc<AtomicUsize>, Arc<Event>, Arc<AtomicUsize>) {
  let dir = TestDir::new(name);
  let input = dir.path().join("main.js");
  std::fs::write(&input, "export const value = 1;\n").expect("write input");

  let dropped = Arc::new(AtomicBool::new(false));
  let bundle_ends = Arc::new(AtomicUsize::new(0));
  let bundle_end_notify = Arc::new(Event::new());
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
    Probe {
      dropped: Arc::clone(&dropped),
      bundle_ends: Arc::clone(&bundle_ends),
      bundle_end_notify: Arc::clone(&bundle_end_notify),
      closes: Arc::clone(&closes),
    },
    &WatcherConfig::default(),
  )
  .unwrap_or_else(|errors| panic!("create watcher: {errors:?}"));
  (dir, watcher, dropped, bundle_ends, bundle_end_notify, closes)
}

#[test]
fn close_on_a_stopped_runtime_fails_without_hanging() {
  let (_dir1, watcher, dropped, _bundle_ends, _bundle_end_notify, closes) =
    create_probe_watcher("rolldown-watcher-stopped-runtime-close");
  let (_dir2, retry_watcher, retry_dropped, retry_bundle_ends, retry_notify, retry_closes) =
    create_probe_watcher("rolldown-watcher-stopped-runtime-retry-run");

  async_runtime::shutdown().expect("stop the shared async runtime");

  // Failed `run()` preserves the coordinator instead of silently dropping it.
  let run_err = watcher.run().expect_err("a stopped runtime must reject run()");
  assert!(
    run_err.to_string().starts_with("Watcher coordinator task submission failed:"),
    "unexpected run error: {run_err}"
  );
  assert!(!dropped.load(Ordering::SeqCst), "a refused run must preserve the coordinator");

  let rejected = futures::executor::block_on(watcher.close())
    .expect_err("a stopped runtime must reject the close submission");
  assert!(
    rejected.to_string().starts_with("Watcher coordinator task submission failed:"),
    "unexpected close error: {rejected}"
  );
  assert_eq!(closes.load(Ordering::SeqCst), 0, "close hooks must not run for a refused close");
  assert!(!dropped.load(Ordering::SeqCst), "a refused close must preserve the coordinator");

  // Second watcher: failed `run()` twice while stopped, then retried after restart.
  retry_watcher.run().expect_err("a stopped runtime must reject run()");
  retry_watcher.run().expect_err("repeated run() on a stopped runtime must still fail");
  assert!(!retry_dropped.load(Ordering::SeqCst), "refused runs must preserve the coordinator");

  // Once the runtime restarts, retrying `close()` spawns the preserved
  // coordinator, emits the `close` event, and releases the coordinator.
  async_runtime::start().expect("restart the shared async runtime");
  futures::executor::block_on(watcher.close()).expect("a later close must resolve");
  assert_eq!(closes.load(Ordering::SeqCst), 1, "retried close must emit the close event");
  drop(watcher);
  assert!(dropped.load(Ordering::SeqCst), "completed close must release the coordinator");

  // And retrying `run()` starts the preserved coordinator and runs the initial build.
  futures::executor::block_on(async {
    let listener = retry_notify.listen();
    retry_watcher.run().expect("retried run() after restart must start the coordinator");
    if retry_bundle_ends.load(Ordering::SeqCst) == 0 {
      listener.await;
    }
    assert!(retry_bundle_ends.load(Ordering::SeqCst) >= 1, "initial build must complete");
    retry_watcher.close().await.expect("close() after retried run must resolve");
  });
  assert_eq!(retry_closes.load(Ordering::SeqCst), 1, "close event must be emitted once");
  drop(retry_watcher);
  assert!(retry_dropped.load(Ordering::SeqCst), "completed close must release the coordinator");
}
