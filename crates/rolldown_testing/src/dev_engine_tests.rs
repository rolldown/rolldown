use rolldown::{
  BundlerOptions, DevModeOptions, ExperimentalOptions,
  plugin::{
    HookBuildStartArgs, HookCloseBundleArgs, HookNoopReturn, HookUsage, Plugin, PluginContext,
  },
};
use rolldown_common::WatcherChangeKind;
use rolldown_dev::{
  BundlerConfig, BundlingFuture, DevCallbackResult, DevEngine, DevOptions, DevWatchOptions,
  RebuildStrategy,
};
use std::{
  borrow::Cow,
  fs,
  future::{Future, poll_fn},
  panic::panic_any,
  path::PathBuf,
  sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicUsize, Ordering},
  },
};
use tokio::{
  sync::{Notify, oneshot},
  time::{Duration, timeout},
};

static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);
const LIVENESS_TIMEOUT: Duration = Duration::from_secs(10);
const BUNDLING_PANIC_MESSAGE: &str = "deterministic bundling task panic";

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

fn gated_panicking_bundling_task(
  entered: Arc<Notify>,
  gate: Arc<CallbackGate>,
) -> impl Future<Output = DevCallbackResult> + Send {
  poll_fn(move |_| {
    entered.notify_one();
    // Intentionally hold the poll so another clone can register as a waiter.
    gate.wait();
    panic_any(BUNDLING_PANIC_MESSAGE.to_string());
  })
}

fn assert_string_panic(error: tokio::task::JoinError) {
  let payload = error.into_panic();
  let message = payload.downcast::<String>().expect("panic payload must remain a String");
  assert_eq!(*message, BUNDLING_PANIC_MESSAGE);
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
            Box::pin(async { Ok(()) })
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
  timeout(LIVENESS_TIMEOUT, hmr_started_rx)
    .await
    .expect("HMR callback must start before the liveness deadline")
    .expect("HMR callback should start");

  let close_engine = Arc::clone(&engine);
  let close_task = tokio::spawn(async move { close_engine.close().await });
  for _ in 0..10 {
    tokio::task::yield_now().await;
  }
  callback_gate.release();

  timeout(LIVENESS_TIMEOUT, async {
    update_task.await.expect("update task should finish");
    close_task.await.expect("close task should finish").expect("dev engine close should succeed");
  })
  .await
  .expect("rebuild and close must finish before the liveness deadline");

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
  timeout(LIVENESS_TIMEOUT, entered.notified())
    .await
    .expect("closeBundle must start before the liveness deadline");
  assert!(engine.is_closed(), "new work must be rejected while close is pending");

  let second_engine = Arc::clone(&engine);
  let second = tokio::spawn(async move { second_engine.close().await });
  tokio::task::yield_now().await;
  assert!(!second.is_finished(), "concurrent close must await terminal cleanup");

  release.notify_one();
  let (first_error, second_error) = timeout(LIVENESS_TIMEOUT, async {
    let first_error = first.await.expect("first close task").expect_err("first close should fail");
    let second_error =
      second.await.expect("second close task").expect_err("second close should fail");
    (first_error, second_error)
  })
  .await
  .expect("all close callers must finish before the liveness deadline");
  assert!(first_error.to_string().contains("dev close terminal failure"));
  assert_eq!(second_error.to_string(), first_error.to_string());
  assert_eq!(calls.load(Ordering::SeqCst), 1);

  let late_error = engine.close().await.expect_err("late close should replay failure");
  assert_eq!(late_error.to_string(), first_error.to_string());
  assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn panicked_bundling_future_wakes_and_replays_to_all_waiters() {
  let entered = Arc::new(Notify::new());
  let gate = Arc::new(CallbackGate { released: Mutex::new(false), release: Condvar::new() });
  let bundling_future = BundlingFuture::new_for_testing(gated_panicking_bundling_task(
    Arc::clone(&entered),
    Arc::clone(&gate),
  ));

  let first_future = bundling_future.clone();
  let first = tokio::spawn(first_future);
  timeout(LIVENESS_TIMEOUT, entered.notified())
    .await
    .expect("the first bundling waiter must enter before the liveness deadline");

  let second_polled = Arc::new(Notify::new());
  let second_polled_in_task = Arc::clone(&second_polled);
  let mut second_future = Box::pin(bundling_future.clone());
  let second = tokio::spawn(async move {
    poll_fn(move |cx| {
      let result = second_future.as_mut().poll(cx);
      second_polled_in_task.notify_one();
      result
    })
    .await
  });
  timeout(LIVENESS_TIMEOUT, second_polled.notified())
    .await
    .expect("the second bundling waiter must poll before the liveness deadline");

  gate.release();

  let (first_error, second_error) = timeout(LIVENESS_TIMEOUT, async {
    let first_error = first.await.expect_err("first waiter must replay the bundling panic");
    let second_error = second.await.expect_err("second waiter must replay the bundling panic");
    (first_error, second_error)
  })
  .await
  .expect("registered bundling waiters must not remain pending");
  assert_string_panic(first_error);
  assert_string_panic(second_error);

  let late_error = timeout(LIVENESS_TIMEOUT, tokio::spawn(bundling_future))
    .await
    .expect("late bundling waiter must not remain pending")
    .expect_err("late waiter must replay the bundling panic");
  assert_string_panic(late_error);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bundling_future_preserves_static_str_panic_payload() {
  const MESSAGE: &str = "static bundling task panic";
  let bundling_future = BundlingFuture::new_for_testing(async { panic_any(MESSAGE) });
  let error =
    tokio::spawn(bundling_future).await.expect_err("the bundling future must replay the panic");
  let payload = error.into_panic();
  let message =
    payload.downcast::<&'static str>().expect("panic payload must remain a static string");
  assert_eq!(*message, MESSAGE);
}

#[derive(Debug)]
struct CallbackPanicPayload(&'static str);

struct NestedPanicPayload(Arc<AtomicUsize>);

impl Drop for NestedPanicPayload {
  fn drop(&mut self) {
    self.0.fetch_add(1, Ordering::SeqCst);
    panic!("nested panic payload destructor panic");
  }
}

struct HostilePanicPayload(Arc<AtomicUsize>);

impl Drop for HostilePanicPayload {
  fn drop(&mut self) {
    self.0.fetch_add(1, Ordering::SeqCst);
    panic_any(NestedPanicPayload(Arc::clone(&self.0)));
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unobserved_opaque_panic_payload_destruction_is_contained() {
  let drops = Arc::new(AtomicUsize::new(0));
  let panic_drops = Arc::clone(&drops);
  let bundling_future = BundlingFuture::new_for_testing(async move {
    panic_any(HostilePanicPayload(panic_drops));
  });

  timeout(LIVENESS_TIMEOUT, bundling_future.clone().drive_for_testing())
    .await
    .expect("the detached driver must publish the panic outcome");
  drop(bundling_future);

  assert_eq!(
    drops.load(Ordering::SeqCst),
    2,
    "both hostile payload destructors must run without escaping teardown"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn close_contains_a_panicked_bundling_future_and_runs_fallback_cleanup() {
  let test_dir = TestDir::new();
  let input = test_dir.0.join("main.js");
  fs::write(&input, "export const value = 1;").expect("write input");

  let close_calls = Arc::new(AtomicUsize::new(0));
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
          build_start_calls: Arc::new(AtomicUsize::new(0)),
          close_calls: Arc::clone(&close_calls),
          close_observed_build_start_calls: Arc::new(AtomicUsize::new(0)),
        })],
      ),
      DevOptions {
        on_output: Some(Arc::new(|_| {
          panic_any(CallbackPanicPayload("consumer output callback panic"))
        })),
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

  let run_engine = Arc::clone(&engine);
  let run = tokio::spawn(async move { run_engine.run().await });
  let run_error = timeout(LIVENESS_TIMEOUT, run)
    .await
    .expect("the run task must finish before the liveness deadline")
    .expect_err("the consumer callback panic must fail the run task");
  let payload = run_error.into_panic();
  let payload = payload
    .downcast::<CallbackPanicPayload>()
    .expect("the first observer must receive the original opaque panic payload");
  assert_eq!(payload.0, "consumer output callback panic");

  let close_error = timeout(LIVENESS_TIMEOUT, engine.close())
    .await
    .expect("close must finish before the liveness deadline")
    .expect_err("close must report the coordinator panic instead of panicking");
  assert!(close_error.to_string().contains("DevEngine coordinator task failed"));
  assert_eq!(close_calls.load(Ordering::SeqCst), 1, "fallback cleanup must run closeBundle");
}
