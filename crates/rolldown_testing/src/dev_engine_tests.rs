use rolldown::{
  BundlerOptions, DevModeOptions, ExperimentalOptions,
  plugin::{
    HookBuildStartArgs, HookCloseBundleArgs, HookNoopReturn, HookUsage, Plugin, PluginContext,
  },
};
use rolldown_common::WatcherChangeKind;
use rolldown_dev::{BundlerConfig, DevEngine, DevOptions, DevWatchOptions, RebuildStrategy};
use std::{
  borrow::Cow,
  fs,
  path::PathBuf,
  sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicUsize, Ordering},
  },
};
use tokio::sync::{Notify, oneshot};

static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
  fn new() -> Self {
    let path = std::env::temp_dir().join(format!(
      "rolldown-dev-close-race-{}-{}",
      std::process::id(),
      NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).expect("create test directory");
    Self(path)
  }
}

impl Drop for TestDir {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

#[derive(Debug)]
struct CallbackGate {
  released: Mutex<bool>,
  release: Condvar,
}

impl CallbackGate {
  fn wait(&self) {
    let released = self.released.lock().expect("callback gate lock poisoned");
    let _guard = self
      .release
      .wait_while(released, |released| !*released)
      .expect("callback gate lock poisoned while waiting");
  }

  fn release(&self) {
    *self.released.lock().expect("callback gate lock poisoned") = true;
    self.release.notify_all();
  }
}

#[derive(Debug)]
struct LifecyclePlugin {
  build_start_calls: Arc<AtomicUsize>,
  close_calls: Arc<AtomicUsize>,
  close_observed_build_start_calls: Arc<AtomicUsize>,
}

impl Plugin for LifecyclePlugin {
  fn name(&self) -> Cow<'static, str> {
    "dev-close-final-handle".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart | HookUsage::CloseBundle
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    self.build_start_calls.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }

  async fn close_bundle(
    &self,
    _ctx: &PluginContext,
    _args: Option<&HookCloseBundleArgs<'_>>,
  ) -> HookNoopReturn {
    self.close_calls.fetch_add(1, Ordering::SeqCst);
    self
      .close_observed_build_start_calls
      .store(self.build_start_calls.load(Ordering::SeqCst), Ordering::SeqCst);
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn close_waits_for_hmr_rebuild_before_closing_the_final_bundle_handle() {
  let test_dir = TestDir::new();
  let input = test_dir.0.join("main.js");
  fs::write(&input, "export const value = 1;").expect("write initial input");

  let build_start_calls = Arc::new(AtomicUsize::new(0));
  let close_calls = Arc::new(AtomicUsize::new(0));
  let close_observed_build_start_calls = Arc::new(AtomicUsize::new(0));
  let (hmr_started_tx, hmr_started_rx) = oneshot::channel();
  let hmr_started_tx = Arc::new(Mutex::new(Some(hmr_started_tx)));
  let build_start_calls_at_hmr = Arc::new(AtomicUsize::new(0));
  let callback_gate =
    Arc::new(CallbackGate { released: Mutex::new(false), release: Condvar::new() });

  let engine = Arc::new(
    DevEngine::new(
      BundlerConfig::new(
        BundlerOptions {
          cwd: Some(test_dir.0.clone()),
          input: Some(vec![input.to_string_lossy().into_owned().into()]),
          experimental: Some(ExperimentalOptions {
            incremental_build: Some(true),
            dev_mode: Some(DevModeOptions::default()),
            ..Default::default()
          }),
          ..Default::default()
        },
        vec![Arc::new(LifecyclePlugin {
          build_start_calls: Arc::clone(&build_start_calls),
          close_calls: Arc::clone(&close_calls),
          close_observed_build_start_calls: Arc::clone(&close_observed_build_start_calls),
        })],
      ),
      DevOptions {
        rebuild_strategy: Some(RebuildStrategy::Always),
        on_hmr_updates: Some({
          let callback_gate = Arc::clone(&callback_gate);
          let hmr_started_tx = Arc::clone(&hmr_started_tx);
          let build_start_calls = Arc::clone(&build_start_calls);
          let build_start_calls_at_hmr = Arc::clone(&build_start_calls_at_hmr);
          Arc::new(move |_| {
            build_start_calls_at_hmr
              .store(build_start_calls.load(Ordering::SeqCst), Ordering::SeqCst);
            let sender = hmr_started_tx.lock().expect("HMR sender lock poisoned").take();
            if let Some(sender) = sender {
              let _ = sender.send(());
            }
            callback_gate.wait();
          })
        }),
        watch: Some(DevWatchOptions {
          disable_watcher: Some(true),
          skip_write: Some(true),
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .expect("create dev engine"),
  );

  engine.run().await.expect("run initial build");
  engine.create_client_for_testing().await;
  fs::write(&input, "export const value = 2;").expect("update input");

  let update_engine = Arc::clone(&engine);
  let changed_input = input.clone();
  let update_task = tokio::spawn(async move {
    update_engine
      .ensure_task_with_changed_files(
        std::iter::once((changed_input, WatcherChangeKind::Update)).collect(),
      )
      .await;
  });
  hmr_started_rx.await.expect("HMR callback should start");

  let close_engine = Arc::clone(&engine);
  let close_task = tokio::spawn(async move { close_engine.close().await });
  for _ in 0..10 {
    tokio::task::yield_now().await;
  }
  callback_gate.release();

  update_task.await.expect("update task should finish");
  close_task.await.expect("close task should finish").expect("dev engine close should succeed");

  assert_eq!(close_calls.load(Ordering::SeqCst), 1);
  assert!(
    close_observed_build_start_calls.load(Ordering::SeqCst)
      > build_start_calls_at_hmr.load(Ordering::SeqCst),
    "closeBundle must observe the rebuild that installs the final bundle handle"
  );
}

#[derive(Debug)]
struct GatedFailingClosePlugin {
  calls: Arc<AtomicUsize>,
  entered: Arc<Notify>,
  release: Arc<Notify>,
}

impl Plugin for GatedFailingClosePlugin {
  fn name(&self) -> Cow<'static, str> {
    "dev-close-failure-replay".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::CloseBundle
  }

  async fn close_bundle(
    &self,
    _ctx: &PluginContext,
    _args: Option<&HookCloseBundleArgs<'_>>,
  ) -> HookNoopReturn {
    self.calls.fetch_add(1, Ordering::SeqCst);
    self.entered.notify_one();
    self.release.notified().await;
    Err(anyhow::anyhow!("dev close terminal failure"))
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_and_late_close_callers_replay_the_terminal_failure() {
  let test_dir = TestDir::new();
  let input = test_dir.0.join("main.js");
  fs::write(&input, "export const value = 1;").expect("write input");

  let calls = Arc::new(AtomicUsize::new(0));
  let entered = Arc::new(Notify::new());
  let release = Arc::new(Notify::new());
  let engine = Arc::new(
    DevEngine::new(
      BundlerConfig::new(
        BundlerOptions {
          cwd: Some(test_dir.0.clone()),
          input: Some(vec![input.to_string_lossy().into_owned().into()]),
          experimental: Some(ExperimentalOptions {
            incremental_build: Some(true),
            dev_mode: Some(DevModeOptions::default()),
            ..Default::default()
          }),
          ..Default::default()
        },
        vec![Arc::new(GatedFailingClosePlugin {
          calls: Arc::clone(&calls),
          entered: Arc::clone(&entered),
          release: Arc::clone(&release),
        })],
      ),
      DevOptions {
        watch: Some(DevWatchOptions {
          disable_watcher: Some(true),
          skip_write: Some(true),
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .expect("create dev engine"),
  );
  engine.run().await.expect("run initial build");

  let first_engine = Arc::clone(&engine);
  let first = tokio::spawn(async move { first_engine.close().await });
  entered.notified().await;
  assert!(engine.is_closed(), "new work must be rejected while close is pending");

  let second_engine = Arc::clone(&engine);
  let second = tokio::spawn(async move { second_engine.close().await });
  tokio::task::yield_now().await;
  assert!(!second.is_finished(), "concurrent close must await terminal cleanup");

  release.notify_waiters();
  let first_error = first.await.expect("first close task").expect_err("first close should fail");
  let second_error =
    second.await.expect("second close task").expect_err("second close should fail");
  assert!(first_error.to_string().contains("dev close terminal failure"));
  assert_eq!(second_error.to_string(), first_error.to_string());
  assert_eq!(calls.load(Ordering::SeqCst), 1);

  let late_error = engine.close().await.expect_err("late close should replay failure");
  assert_eq!(late_error.to_string(), first_error.to_string());
  assert_eq!(calls.load(Ordering::SeqCst), 1);
}
