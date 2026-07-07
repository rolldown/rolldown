use crate::handler::WatcherEventHandler;
use crate::task_fs_event_handler::TaskFsEventHandler;
use crate::watch_coordinator::{CoordinatorCloseError, CoordinatorCloseResult, WatchCoordinator};
use crate::watch_task::{WatchTask, WatchTaskIdx};
use crate::watcher_msg::WatcherMsg;
use anyhow::Result;
use futures::FutureExt;
use futures::future::Shared;
use oxc_index::IndexVec;
use rolldown::BundlerConfig;
use rolldown_error::BuildResult;
use rolldown_fs_watcher::FsWatcherConfig;
use rolldown_utils::futures::spawn;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::{Notify, mpsc};

/// Default debounce duration in milliseconds.
/// Matches Rollup's default buildDelay of 0ms.
const DEFAULT_DEBOUNCE_MS: u64 = 0;

/// Configuration for the watcher
#[derive(Debug, Clone, Default)]
pub struct WatcherConfig {
  /// Debounce duration for file changes
  pub debounce: Option<std::time::Duration>,
  /// Whether to use polling-based file watching instead of native OS events
  pub use_polling: bool,
  /// Poll interval in milliseconds (only used when `use_polling` is true)
  pub poll_interval: Option<u64>,
  /// Whether to compare file contents for poll-based watchers (only used when `use_polling` is true)
  pub compare_contents_for_polling: bool,
  /// Whether to use debounced event delivery at the filesystem level
  pub use_debounce: bool,
  /// Debounce delay in milliseconds for fs-level debounced watchers (only used when `use_debounce` is true)
  pub debounce_delay: Option<u64>,
  /// Tick rate in milliseconds for the debouncer's internal polling (only used when `use_debounce` is true)
  pub debounce_tick_rate: Option<u64>,
}

impl WatcherConfig {
  pub fn debounce_duration(&self) -> std::time::Duration {
    self.debounce.unwrap_or(std::time::Duration::from_millis(DEFAULT_DEBOUNCE_MS))
  }

  fn to_fs_watcher_config(&self) -> FsWatcherConfig {
    let mut config = FsWatcherConfig::default();
    if let Some(poll_interval) = self.poll_interval {
      config.poll_interval = poll_interval;
    }
    config.compare_contents_for_polling = self.compare_contents_for_polling;
    config.use_polling = self.use_polling;
    config.use_debounce = self.use_debounce;
    if let Some(debounce_delay) = self.debounce_delay {
      config.debounce_delay = debounce_delay;
    }
    config.debounce_tick_rate = self.debounce_tick_rate;
    config
  }
}

type CoordinatorFuture = Shared<Pin<Box<dyn Future<Output = CoordinatorCloseResult> + Send>>>;

#[derive(Debug)]
struct SharedCoordinatorCloseError(Arc<CoordinatorCloseError>);

impl fmt::Display for SharedCoordinatorCloseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(f)
  }
}

impl std::error::Error for SharedCoordinatorCloseError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(self.0.as_ref())
  }
}

struct CoordinatorState {
  /// The coordinator future, before `run()` is called.
  coordinator: Option<Pin<Box<dyn Future<Output = CoordinatorCloseResult> + Send>>>,
  /// The spawned handle, after `run()` is called. Shared so multiple callers can await.
  handle: Option<CoordinatorFuture>,
}

/// The main watcher that manages multiple bundlers.
///
/// Usage: `Watcher::new(configs, handler, &config)` → `watcher.run()` → `watcher.close()`.
pub struct Watcher {
  coordinator_state: std::sync::Mutex<CoordinatorState>,
  tx: mpsc::UnboundedSender<WatcherMsg>,
  closed: Arc<AtomicBool>,
  close_notify: Arc<Notify>,
}

impl Watcher {
  /// Create a new watcher with multiple bundler configs and a handler.
  /// The coordinator future is created but not spawned — call `run()` to start.
  pub fn new<H: WatcherEventHandler + 'static>(
    configs: Vec<BundlerConfig>,
    handler: H,
    watcher_config: &WatcherConfig,
  ) -> BuildResult<Self> {
    let (tx, rx) = mpsc::unbounded_channel();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Notify::new());
    let tasks = Self::create_tasks(configs, watcher_config, &tx, &closed)?;
    let coordinator = WatchCoordinator::new(
      rx,
      handler,
      tasks,
      watcher_config,
      Arc::clone(&closed),
      Arc::clone(&close_notify),
    );
    let coordinator_future: Pin<Box<dyn Future<Output = CoordinatorCloseResult> + Send>> =
      Box::pin(coordinator.run());

    Ok(Self {
      coordinator_state: std::sync::Mutex::new(CoordinatorState {
        coordinator: Some(coordinator_future),
        handle: None,
      }),
      tx,
      closed,
      close_notify,
    })
  }

  /// Spawn the coordinator. Can only be called once.
  pub fn run(&self) {
    let mut state = self.coordinator_state.lock().unwrap();
    if let Some(coordinator) = state.coordinator.take() {
      let join_handle = spawn(coordinator);
      let handle: Pin<Box<dyn Future<Output = CoordinatorCloseResult> + Send>> =
        Box::pin(async move {
          match join_handle.await {
            Ok(result) => result,
            Err(error) => Err(Arc::new(CoordinatorCloseError::from_message(format!(
              "Watcher coordinator task failed: {error}"
            )))),
          }
        });
      state.handle = Some(handle.shared());
    }
  }

  /// Gives consumers a reliable way to await the watcher's completion.
  pub async fn wait_for_close(&self) {
    let handle = self.coordinator_state.lock().unwrap().handle.clone();
    if let Some(handle) = handle {
      let _ = handle.await;
    }
  }

  /// Close the watcher and wait for the coordinator to finish. Closing before
  /// the first scheduled `run()` still starts the coordinator so plugin and
  /// handler cleanup runs through the normal state machine.
  pub async fn close(&self) -> Result<()> {
    self.closed.store(true, std::sync::atomic::Ordering::Relaxed);
    // Publish close before spawning a not-yet-started coordinator. Otherwise
    // a pool worker could enter the initial build between `run()` and this
    // store, making same-tick close nondeterministically start a bundle.
    self.run();
    // Wake the coordinator even when it is waiting for a user event callback. The mpsc message
    // remains the normal state-machine input when the coordinator is idle or debouncing.
    self.close_notify.notify_one();
    let _ = self.tx.send(WatcherMsg::Close);
    let handle = self.coordinator_state.lock().unwrap().handle.clone();
    match handle {
      Some(handle) => {
        handle.await.map_err(|error| anyhow::Error::new(SharedCoordinatorCloseError(error)))
      }
      None => Ok(()),
    }
  }

  fn create_tasks(
    configs: Vec<BundlerConfig>,
    watcher_config: &WatcherConfig,
    tx: &mpsc::UnboundedSender<WatcherMsg>,
    closed: &Arc<AtomicBool>,
  ) -> BuildResult<IndexVec<WatchTaskIdx, WatchTask>> {
    let fs_watcher_config = watcher_config.to_fs_watcher_config();
    let mut tasks = IndexVec::with_capacity(configs.len());
    for (index, config) in configs.into_iter().enumerate() {
      let task_index = WatchTaskIdx::from_usize(index);
      let fs_handler = TaskFsEventHandler { task_index, tx: tx.clone() };
      let fs_watcher =
        rolldown_fs_watcher::create_fs_watcher(fs_handler, fs_watcher_config.clone())?;
      let task = WatchTask::new(config, fs_watcher, closed)?;
      tasks.push(task);
    }
    Ok(tasks)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::WatchEvent;
  use rolldown::{BundlerOptions, plugin};
  use rolldown_common::WatcherChangeKind;
  use std::{
    borrow::Cow,
    panic::panic_any,
    sync::{
      Arc,
      atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
  };
  use tokio::sync::Notify;

  struct RecordingHandler {
    end: Arc<Notify>,
    close_calls: Arc<AtomicUsize>,
  }

  impl WatcherEventHandler for RecordingHandler {
    async fn on_event(&self, event: WatchEvent) {
      if matches!(event, WatchEvent::End) {
        self.end.notify_one();
      }
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {}

    async fn on_restart(&self) {}

    async fn on_close(&self) {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
    }
  }

  struct PanickingCloseHandler {
    end: Arc<Notify>,
    close_calls: Arc<AtomicUsize>,
  }

  impl WatcherEventHandler for PanickingCloseHandler {
    async fn on_event(&self, event: WatchEvent) {
      if matches!(event, WatchEvent::End) {
        self.end.notify_one();
      }
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {}

    async fn on_restart(&self) {}

    async fn on_close(&self) {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
      panic!("intentional close event panic");
    }
  }

  struct PanickingBuildEventHandler {
    close_calls: Arc<AtomicUsize>,
    close_bundle_calls: Arc<AtomicUsize>,
    close_bundle_calls_before_panic: Arc<AtomicUsize>,
    panic_payload_drops: Arc<AtomicUsize>,
  }

  impl WatcherEventHandler for PanickingBuildEventHandler {
    async fn on_event(&self, event: WatchEvent) {
      if matches!(event, WatchEvent::BundleEnd(_) | WatchEvent::Error(_)) {
        self
          .close_bundle_calls_before_panic
          .store(self.close_bundle_calls.load(Ordering::SeqCst), Ordering::SeqCst);
        panic_any(HostilePanicPayload(Arc::clone(&self.panic_payload_drops)));
      }
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {}

    async fn on_restart(&self) {}

    async fn on_close(&self) {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
    }
  }

  struct HostilePanicPayload(Arc<AtomicUsize>);

  impl Drop for HostilePanicPayload {
    fn drop(&mut self) {
      self.0.fetch_add(1, Ordering::SeqCst);
      panic!("intentional panic payload destructor panic");
    }
  }

  #[derive(Debug)]
  struct FailingClosePlugin {
    message: &'static str,
    close_watcher_calls: Arc<AtomicUsize>,
    close_bundle_calls: Arc<AtomicUsize>,
  }

  impl plugin::Plugin for FailingClosePlugin {
    fn name(&self) -> Cow<'static, str> {
      self.message.into()
    }

    fn register_hook_usage(&self) -> plugin::HookUsage {
      plugin::HookUsage::CloseWatcher | plugin::HookUsage::CloseBundle
    }

    async fn close_watcher(&self, _ctx: &plugin::PluginContext) -> plugin::HookNoopReturn {
      self.close_watcher_calls.fetch_add(1, Ordering::SeqCst);
      Err(anyhow::anyhow!("{} closeWatcher", self.message))
    }

    async fn close_bundle(
      &self,
      _ctx: &plugin::PluginContext,
      _args: Option<&plugin::HookCloseBundleArgs<'_>>,
    ) -> plugin::HookNoopReturn {
      self.close_bundle_calls.fetch_add(1, Ordering::SeqCst);
      Err(anyhow::anyhow!("{} closeBundle", self.message))
    }
  }

  #[derive(Debug)]
  struct CleanupProbePlugin {
    name: &'static str,
    panic_close_watcher: bool,
    close_watcher_calls: Arc<AtomicUsize>,
    close_bundle_calls: Arc<AtomicUsize>,
  }

  impl plugin::Plugin for CleanupProbePlugin {
    fn name(&self) -> Cow<'static, str> {
      self.name.into()
    }

    fn register_hook_usage(&self) -> plugin::HookUsage {
      plugin::HookUsage::CloseWatcher | plugin::HookUsage::CloseBundle
    }

    async fn close_watcher(&self, _ctx: &plugin::PluginContext) -> plugin::HookNoopReturn {
      self.close_watcher_calls.fetch_add(1, Ordering::SeqCst);
      assert!(!self.panic_close_watcher, "intentional closeWatcher panic");
      Ok(())
    }

    async fn close_bundle(
      &self,
      _ctx: &plugin::PluginContext,
      _args: Option<&plugin::HookCloseBundleArgs<'_>>,
    ) -> plugin::HookNoopReturn {
      self.close_bundle_calls.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  #[test]
  fn test_watcher_config_default_debounce() {
    let config = WatcherConfig::default();
    assert_eq!(config.debounce_duration(), Duration::from_millis(DEFAULT_DEBOUNCE_MS));
  }

  #[test]
  fn test_watcher_config_custom_debounce() {
    let config = WatcherConfig { debounce: Some(Duration::from_millis(500)), ..Default::default() };
    assert_eq!(config.debounce_duration(), Duration::from_millis(500));
  }

  #[test]
  fn test_fs_watcher_config_defaults() {
    let config = WatcherConfig::default();
    let fs_config = config.to_fs_watcher_config();
    assert_eq!(fs_config.poll_interval, 100);
  }

  #[test]
  fn test_fs_watcher_config_with_poll_interval() {
    let config = WatcherConfig { poll_interval: Some(250), ..Default::default() };
    let fs_config = config.to_fs_watcher_config();
    assert_eq!(fs_config.poll_interval, 250);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn close_aggregates_hook_errors_and_replays_them_to_later_callers() {
    let close_watcher_calls = Arc::new(AtomicUsize::new(0));
    let close_bundle_calls = Arc::new(AtomicUsize::new(0));
    let handler_close_calls = Arc::new(AtomicUsize::new(0));
    let end = Arc::new(Notify::new());
    let configs = ["first close failure", "second close failure"]
      .into_iter()
      .map(|message| {
        BundlerConfig::new(
          BundlerOptions::default(),
          vec![Arc::new(FailingClosePlugin {
            message,
            close_watcher_calls: Arc::clone(&close_watcher_calls),
            close_bundle_calls: Arc::clone(&close_bundle_calls),
          })],
        )
      })
      .collect();
    let watcher = Watcher::new(
      configs,
      RecordingHandler { end: Arc::clone(&end), close_calls: Arc::clone(&handler_close_calls) },
      &WatcherConfig::default(),
    )
    .expect("create watcher");
    watcher.run();
    end.notified().await;
    let close_bundle_calls_before_shutdown = close_bundle_calls.load(Ordering::SeqCst);

    let (first, concurrent) = tokio::join!(watcher.close(), watcher.close());
    let first_error = first.expect_err("close should fail").to_string();
    let concurrent_error = concurrent.expect_err("concurrent close should fail").to_string();
    assert_eq!(concurrent_error, first_error);
    assert!(first_error.contains("watch task 0 closeWatcher failed"));
    assert!(first_error.contains("first close failure closeWatcher"));
    assert!(first_error.contains("watch task 1 closeWatcher failed"));
    assert!(first_error.contains("second close failure closeWatcher"));
    assert!(first_error.contains("watch task 0 closeBundle failed"));
    assert!(first_error.contains("first close failure closeBundle"));
    assert!(first_error.contains("watch task 1 closeBundle failed"));
    assert!(first_error.contains("second close failure closeBundle"));
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_shutdown + 2);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);

    let second_error = watcher.close().await.expect_err("later close should replay the failure");
    assert_eq!(second_error.to_string(), first_error);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_shutdown + 2);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn close_contains_panics_and_finishes_every_cleanup_phase() {
    let close_watcher_calls = Arc::new(AtomicUsize::new(0));
    let close_bundle_calls = Arc::new(AtomicUsize::new(0));
    let handler_close_calls = Arc::new(AtomicUsize::new(0));
    let end = Arc::new(Notify::new());
    let configs = [("panicking", true), ("following", false)]
      .into_iter()
      .map(|(name, panic_close_watcher)| {
        BundlerConfig::new(
          BundlerOptions::default(),
          vec![Arc::new(CleanupProbePlugin {
            name,
            panic_close_watcher,
            close_watcher_calls: Arc::clone(&close_watcher_calls),
            close_bundle_calls: Arc::clone(&close_bundle_calls),
          })],
        )
      })
      .collect();
    let watcher = Watcher::new(
      configs,
      PanickingCloseHandler {
        end: Arc::clone(&end),
        close_calls: Arc::clone(&handler_close_calls),
      },
      &WatcherConfig::default(),
    )
    .expect("create watcher");
    watcher.run();
    end.notified().await;
    let close_bundle_calls_before_shutdown = close_bundle_calls.load(Ordering::SeqCst);

    let first_error = watcher.close().await.expect_err("close should report contained panics");
    let first_message = first_error.to_string();
    assert!(first_message.contains("watch task 0 closeWatcher failed"));
    assert!(first_message.contains("intentional closeWatcher panic"));
    assert!(first_message.contains("watch close event handler failed"));
    assert!(first_message.contains("intentional close event panic"));
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_shutdown + 2);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);

    let replayed = watcher.close().await.expect_err("later close should replay the panic result");
    assert_eq!(replayed.to_string(), first_message);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_shutdown + 2);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn event_panic_runs_cleanup_once_and_aggregates_cleanup_failures() {
    let close_watcher_calls = Arc::new(AtomicUsize::new(0));
    let close_bundle_calls = Arc::new(AtomicUsize::new(0));
    let close_bundle_calls_before_panic = Arc::new(AtomicUsize::new(usize::MAX));
    let handler_close_calls = Arc::new(AtomicUsize::new(0));
    let panic_payload_drops = Arc::new(AtomicUsize::new(0));
    let watcher = Watcher::new(
      vec![BundlerConfig::new(
        BundlerOptions::default(),
        vec![Arc::new(FailingClosePlugin {
          message: "cleanup failure",
          close_watcher_calls: Arc::clone(&close_watcher_calls),
          close_bundle_calls: Arc::clone(&close_bundle_calls),
        })],
      )],
      PanickingBuildEventHandler {
        close_calls: Arc::clone(&handler_close_calls),
        close_bundle_calls: Arc::clone(&close_bundle_calls),
        close_bundle_calls_before_panic: Arc::clone(&close_bundle_calls_before_panic),
        panic_payload_drops: Arc::clone(&panic_payload_drops),
      },
      &WatcherConfig::default(),
    )
    .expect("create watcher");
    watcher.run();
    watcher.wait_for_close().await;

    let first_error = watcher.close().await.expect_err("event panic should fail watcher close");
    let first_message = first_error.to_string();
    assert!(first_message.contains("watch coordinator event loop panicked"));
    assert!(first_message.contains("non-string panic payload"));
    assert!(first_message.contains("watch task 0 closeWatcher failed"));
    assert!(first_message.contains("cleanup failure closeWatcher"));
    assert!(first_message.contains("watch task 0 closeBundle failed"));
    assert!(first_message.contains("cleanup failure closeBundle"));
    assert_eq!(panic_payload_drops.load(Ordering::SeqCst), 1);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 1);
    let close_bundle_calls_before_panic = close_bundle_calls_before_panic.load(Ordering::SeqCst);
    assert_ne!(close_bundle_calls_before_panic, usize::MAX);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_panic + 1);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);

    let replayed = watcher.close().await.expect_err("later close should replay the panic result");
    assert_eq!(replayed.to_string(), first_message);
    assert_eq!(panic_payload_drops.load(Ordering::SeqCst), 1);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_panic + 1);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);
  }
}
