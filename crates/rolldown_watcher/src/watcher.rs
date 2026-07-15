use crate::handler::WatcherEventHandler;
use crate::task_fs_event_handler::TaskFsEventHandler;
use crate::watch_coordinator::{CoordinatorCloseError, CoordinatorCloseResult, WatchCoordinator};
use crate::watch_task::{WatchTask, WatchTaskIdx};
use crate::watcher_msg::WatcherMsg;
use anyhow::Result;
use event_listener::Event;
use futures::FutureExt;
use futures::channel::mpsc;
use futures::future::Shared;
use oxc_index::IndexVec;
use rolldown::BundlerConfig;
use rolldown_error::BuildResult;
use rolldown_fs_watcher::FsWatcherConfig;
use rolldown_utils::futures::try_spawn;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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

/// Returned when the selected async runtime rejects watcher coordinator
/// submission before the coordinator starts.
#[derive(Debug)]
pub struct WatcherStartError {
  source: Box<dyn std::error::Error + Send + Sync>,
}

impl WatcherStartError {
  fn new(error: impl std::error::Error + Send + Sync + 'static) -> Self {
    Self { source: Box::new(error) }
  }
}

impl fmt::Display for WatcherStartError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Watcher coordinator task submission failed: {}", self.source)
  }
}

impl std::error::Error for WatcherStartError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(self.source.as_ref())
  }
}

type PendingCoordinatorFuture = Pin<Box<dyn Future<Output = CoordinatorCloseResult> + Send>>;
type CoordinatorFuture = Shared<PendingCoordinatorFuture>;

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
  coordinator: Option<PendingCoordinatorFuture>,
  /// The spawned handle, after `run()` is called. Shared so multiple callers can await.
  handle: Option<CoordinatorFuture>,
}

impl CoordinatorState {
  // See internal-docs/watch-mode/implementation.md for retry ownership.
  fn try_start<E>(
    &mut self,
    start: impl FnOnce(
      PendingCoordinatorFuture,
    ) -> Result<CoordinatorFuture, (E, PendingCoordinatorFuture)>,
  ) -> Result<(), E> {
    let Some(coordinator) = self.coordinator.take() else {
      return Ok(());
    };

    match start(coordinator) {
      Ok(handle) => {
        self.handle = Some(handle);
        Ok(())
      }
      Err((error, coordinator)) => {
        self.coordinator = Some(coordinator);
        Err(error)
      }
    }
  }
}

/// The main watcher that manages multiple bundlers.
///
/// Usage: `Watcher::new(configs, handler, &config)` → `watcher.run()` → `watcher.close()`.
pub struct Watcher {
  coordinator_state: std::sync::Mutex<CoordinatorState>,
  tx: mpsc::UnboundedSender<WatcherMsg>,
  closed: Arc<AtomicBool>,
  close_notify: Arc<Event>,
  native_owned_close_identities: Arc<std::sync::Mutex<Vec<u64>>>,
}

impl Watcher {
  /// Create a new watcher with multiple bundler configs and a handler.
  /// The coordinator future is created but not spawned — call `run()` to start.
  pub fn new<H: WatcherEventHandler + 'static>(
    configs: Vec<BundlerConfig>,
    handler: H,
    watcher_config: &WatcherConfig,
  ) -> BuildResult<Self> {
    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());
    let native_owned_close_identities = Arc::new(std::sync::Mutex::new(Vec::new()));
    let tasks = Self::create_tasks(configs, watcher_config, &tx, &closed)?;
    let coordinator = WatchCoordinator::new(
      rx,
      handler,
      tasks,
      watcher_config,
      Arc::clone(&closed),
      Arc::clone(&close_notify),
      Arc::clone(&native_owned_close_identities),
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
      native_owned_close_identities,
    })
  }

  /// Spawn the coordinator. Accepted and completed starts are idempotent; a
  /// runtime-rejected start retains the coordinator for a later retry.
  pub fn run(&self) -> Result<(), WatcherStartError> {
    self.start_coordinator(|coordinator| match try_spawn(coordinator) {
      Ok(join_handle) => {
        let handle: PendingCoordinatorFuture = Box::pin(async move {
          match join_handle.await {
            Ok(result) => result,
            Err(error) => Err(Arc::new(CoordinatorCloseError::from_message(format!(
              "Watcher coordinator task failed: {error}"
            )))),
          }
        });
        Ok(handle.shared())
      }
      Err((error, coordinator)) => Err((error, coordinator)),
    })
  }

  fn start_coordinator<E: std::error::Error + Send + Sync + 'static>(
    &self,
    start: impl FnOnce(
      PendingCoordinatorFuture,
    ) -> Result<CoordinatorFuture, (E, PendingCoordinatorFuture)>,
  ) -> Result<(), WatcherStartError> {
    let result = self.coordinator_state.lock().unwrap().try_start(start);
    result.map_err(WatcherStartError::new)
  }

  /// Gives consumers a reliable way to await the watcher's completion.
  pub async fn wait_for_close(&self) {
    let handle = self.coordinator_state.lock().unwrap().handle.clone();
    if let Some(handle) = handle {
      let _ = handle.await;
    }
  }

  /// Publish a close request without spawning or awaiting the coordinator.
  ///
  /// This is safe to call directly from a host callback that is not currently
  /// entered through the selected async runtime.
  pub fn publish_close(&self) {
    if self.closed.swap(true, std::sync::atomic::Ordering::Relaxed) {
      return;
    }
    // Wake the coordinator even when it is waiting for a user event callback. The mpsc message
    // remains the normal state-machine input when the coordinator is idle or debouncing.
    // `event_listener::Event` stores no permit, so this must run after the `closed` flag is set
    // above: any waiter that created its listener before this call is woken, and a waiter that
    // created its listener after this call observes `closed == true` and skips waiting.
    self.close_notify.notify(usize::MAX);
    let _ = self.tx.unbounded_send(WatcherMsg::Close);
  }

  /// Close the watcher and wait for the coordinator to finish.
  pub async fn close(&self) -> Result<()> {
    // Publish close before spawning a not-yet-started coordinator. Otherwise
    // a pool worker could enter the initial build between `run()` and the
    // close signal, making same-tick close nondeterministically start a bundle.
    self.publish_close();
    self.run().map_err(anyhow::Error::new)?;
    let handle = self.coordinator_state.lock().unwrap().handle.clone();
    match handle {
      Some(handle) => {
        handle.await.map_err(|error| anyhow::Error::new(SharedCoordinatorCloseError(error)))
      }
      None => Ok(()),
    }
  }

  #[doc(hidden)]
  pub fn native_owned_close_identities(&self) -> Vec<u64> {
    self
      .native_owned_close_identities
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .clone()
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
  use crate::{CoordinatorCloseFailure, FileChangeEvent, WatchEvent};
  use rolldown::{BundlerOptions, plugin};
  use rolldown_common::WatcherChangeKind;
  use std::{
    borrow::Cow,
    convert::Infallible,
    fs,
    panic::panic_any,
    path::PathBuf,
    sync::{
      Arc,
      atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
  };
  // `event_listener::Event` backs the `Watcher::close_notify` production field.
  // `tokio::sync::Notify` is kept ONLY for the tests' internal `end` signal.
  use event_listener::Event;
  use tokio::sync::Notify;

  static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

  struct TestDir(PathBuf);

  impl TestDir {
    fn new() -> Self {
      let path = std::env::temp_dir().join(format!(
        "rolldown-watcher-lifecycle-{}-{}",
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

  struct RecordingHandler {
    end: Arc<Notify>,
    close_calls: Arc<AtomicUsize>,
  }

  impl WatcherEventHandler for RecordingHandler {
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
      if matches!(event, WatchEvent::End) {
        self.end.notify_one();
      }
      Ok(())
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_restart(&self) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_close(&self) -> anyhow::Result<()> {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  struct PanickingCloseHandler {
    end: Arc<Notify>,
    close_calls: Arc<AtomicUsize>,
  }

  impl WatcherEventHandler for PanickingCloseHandler {
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
      if matches!(event, WatchEvent::End) {
        self.end.notify_one();
      }
      Ok(())
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_restart(&self) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_close(&self) -> anyhow::Result<()> {
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

  struct FailingBuildEventHandler {
    close_calls: Arc<AtomicUsize>,
  }

  impl WatcherEventHandler for FailingBuildEventHandler {
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
      if matches!(event, WatchEvent::BundleEnd(_) | WatchEvent::Error(_)) {
        anyhow::bail!("intentional event listener failure");
      }
      Ok(())
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_restart(&self) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_close(&self) -> anyhow::Result<()> {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  impl WatcherEventHandler for PanickingBuildEventHandler {
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
      if matches!(event, WatchEvent::BundleEnd(_) | WatchEvent::Error(_)) {
        self
          .close_bundle_calls_before_panic
          .store(self.close_bundle_calls.load(Ordering::SeqCst), Ordering::SeqCst);
        panic_any(HostilePanicPayload(Arc::clone(&self.panic_payload_drops)));
      }
      Ok(())
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_restart(&self) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_close(&self) -> anyhow::Result<()> {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
      Ok(())
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

  #[derive(Debug)]
  struct FailingWatchChangePlugin {
    watch_change_calls: Arc<AtomicUsize>,
    close_watcher_calls: Arc<AtomicUsize>,
  }

  impl plugin::Plugin for FailingWatchChangePlugin {
    fn name(&self) -> Cow<'static, str> {
      "failing-watch-change".into()
    }

    fn register_hook_usage(&self) -> plugin::HookUsage {
      plugin::HookUsage::WatchChange | plugin::HookUsage::CloseWatcher
    }

    async fn watch_change(
      &self,
      _ctx: &plugin::PluginContext,
      _path: &str,
      _event: WatcherChangeKind,
    ) -> plugin::HookNoopReturn {
      self.watch_change_calls.fetch_add(1, Ordering::SeqCst);
      Err(anyhow::anyhow!("intentional watchChange failure"))
    }

    async fn close_watcher(&self, _ctx: &plugin::PluginContext) -> plugin::HookNoopReturn {
      self.close_watcher_calls.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
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

  #[tokio::test]
  async fn rejected_coordinator_submission_is_retryable_after_runtime_restart() {
    let runs = Arc::new(AtomicUsize::new(0));
    let runs_task = Arc::clone(&runs);
    let coordinator: PendingCoordinatorFuture = Box::pin(async move {
      runs_task.fetch_add(1, Ordering::SeqCst);
      Ok(())
    });
    let (tx, _rx) = mpsc::unbounded();
    let watcher = Watcher {
      coordinator_state: std::sync::Mutex::new(CoordinatorState {
        coordinator: Some(coordinator),
        handle: None,
      }),
      tx,
      closed: Arc::new(AtomicBool::new(false)),
      close_notify: Arc::new(Event::new()),
      native_owned_close_identities: Arc::new(std::sync::Mutex::new(Vec::new())),
    };

    let error = watcher
      .start_coordinator(|coordinator| Err((std::io::Error::other("runtime stopped"), coordinator)))
      .expect_err("the first run must reject while the runtime is stopped");
    assert_eq!(error.to_string(), "Watcher coordinator task submission failed: runtime stopped");
    assert_eq!(
      std::error::Error::source(&error).map(std::string::ToString::to_string).as_deref(),
      Some("runtime stopped")
    );
    {
      let state = watcher.coordinator_state.lock().unwrap();
      assert!(state.coordinator.is_some());
      assert!(state.handle.is_none());
    }
    assert_eq!(runs.load(Ordering::SeqCst), 0);

    let accepted_submissions = Arc::new(AtomicUsize::new(0));
    let accepted_submissions_task = Arc::clone(&accepted_submissions);
    watcher
      .start_coordinator::<Infallible>(|coordinator| {
        accepted_submissions_task.fetch_add(1, Ordering::SeqCst);
        Ok(coordinator.shared())
      })
      .expect("a restarted runtime must accept the retained coordinator");
    watcher
      .start_coordinator::<Infallible>(|coordinator| {
        accepted_submissions.fetch_add(1, Ordering::SeqCst);
        Ok(coordinator.shared())
      })
      .expect("an accepted start must be idempotent");
    let handle = watcher
      .coordinator_state
      .lock()
      .unwrap()
      .handle
      .clone()
      .expect("accepted coordinator must publish its handle");
    handle.await.expect("retained coordinator must complete");
    let state = watcher.coordinator_state.lock().unwrap();
    assert!(state.coordinator.is_none());
    assert_eq!(accepted_submissions.load(Ordering::SeqCst), 1);
    assert_eq!(runs.load(Ordering::SeqCst), 1);
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
    watcher.run().expect("start watcher");
    end.notified().await;
    let close_bundle_calls_before_shutdown = close_bundle_calls.load(Ordering::SeqCst);
    assert_eq!(close_bundle_calls_before_shutdown, 2);

    let (first, concurrent) = tokio::join!(watcher.close(), watcher.close());
    let first_error = first.expect_err("close should fail");
    let concurrent_error = concurrent.expect_err("concurrent close should fail");
    let first_coordinator_error = &first_error
      .downcast_ref::<SharedCoordinatorCloseError>()
      .expect("coordinator close error")
      .0;
    let concurrent_coordinator_error = &concurrent_error
      .downcast_ref::<SharedCoordinatorCloseError>()
      .expect("coordinator close error")
      .0;
    assert!(Arc::ptr_eq(first_coordinator_error, concurrent_coordinator_error));
    let first_message = first_error.to_string();
    assert_eq!(concurrent_error.to_string(), first_message);
    let failures = first_coordinator_error.failures();
    assert_eq!(failures.len(), 4);
    assert!(failures[0].message().starts_with("watch task 0 closeWatcher failed:"));
    assert!(failures[0].message().contains("first close failure closeWatcher"));
    assert!(failures[1].message().starts_with("watch task 1 closeWatcher failed:"));
    assert!(failures[1].message().contains("second close failure closeWatcher"));
    assert!(failures[2].message().starts_with("watch task 0 closeBundle failed:"));
    assert!(failures[2].message().contains("first close failure closeBundle"));
    assert!(failures[3].message().starts_with("watch task 1 closeBundle failed:"));
    assert!(failures[3].message().contains("second close failure closeBundle"));
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_shutdown);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);

    let second_error = watcher.close().await.expect_err("later close should replay the failure");
    let second_coordinator_error = &second_error
      .downcast_ref::<SharedCoordinatorCloseError>()
      .expect("coordinator close error")
      .0;
    assert!(Arc::ptr_eq(first_coordinator_error, second_coordinator_error));
    assert_eq!(second_error.to_string(), first_message);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_shutdown);
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
    watcher.run().expect("start watcher");
    end.notified().await;
    let close_bundle_calls_before_shutdown = close_bundle_calls.load(Ordering::SeqCst);
    assert_eq!(close_bundle_calls_before_shutdown, 2);

    let first_error = watcher.close().await.expect_err("close should report contained panics");
    let first_message = first_error.to_string();
    let coordinator_error = &first_error
      .downcast_ref::<SharedCoordinatorCloseError>()
      .expect("coordinator close error")
      .0;
    assert_eq!(
      coordinator_error.failures().iter().map(CoordinatorCloseFailure::message).collect::<Vec<_>>(),
      [
        "watch task 0 closeWatcher failed: intentional closeWatcher panic",
        "watch close event handler failed: intentional close event panic",
      ]
    );
    assert!(first_message.contains("watch task 0 closeWatcher failed"));
    assert!(first_message.contains("intentional closeWatcher panic"));
    assert!(first_message.contains("watch close event handler failed"));
    assert!(first_message.contains("intentional close event panic"));
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_shutdown);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);

    let replayed = watcher.close().await.expect_err("later close should replay the panic result");
    let replayed_coordinator_error =
      &replayed.downcast_ref::<SharedCoordinatorCloseError>().expect("coordinator close error").0;
    assert!(Arc::ptr_eq(coordinator_error, replayed_coordinator_error));
    assert_eq!(replayed.to_string(), first_message);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_shutdown);
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
    watcher.run().expect("start watcher");
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
    assert_eq!(close_bundle_calls_before_panic, 1);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_panic);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);

    let replayed = watcher.close().await.expect_err("later close should replay the panic result");
    assert_eq!(replayed.to_string(), first_message);
    assert_eq!(panic_payload_drops.load(Ordering::SeqCst), 1);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), close_bundle_calls_before_panic);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn event_listener_error_runs_cleanup_and_replays_through_close() {
    let handler_close_calls = Arc::new(AtomicUsize::new(0));
    let watcher = Watcher::new(
      vec![BundlerConfig::new(BundlerOptions::default(), vec![])],
      FailingBuildEventHandler { close_calls: Arc::clone(&handler_close_calls) },
      &WatcherConfig::default(),
    )
    .expect("create watcher");
    watcher.run().expect("start watcher");
    watcher.wait_for_close().await;

    let first_error = watcher.close().await.expect_err("event listener failure should fail close");
    let first_message = first_error.to_string();
    assert!(first_message.contains("watch event listener failed"));
    assert!(first_message.contains("intentional event listener failure"));
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);

    let replayed = watcher.close().await.expect_err("later close should replay listener failure");
    assert_eq!(replayed.to_string(), first_message);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn watch_change_error_runs_cleanup_and_replays_through_close() {
    let test_dir = TestDir::new();
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write input");
    let input = fs::canonicalize(input).expect("canonicalize input");
    let cwd = input.parent().expect("input has parent").to_path_buf();

    let watch_change_calls = Arc::new(AtomicUsize::new(0));
    let close_watcher_calls = Arc::new(AtomicUsize::new(0));
    let handler_close_calls = Arc::new(AtomicUsize::new(0));
    let end = Arc::new(Notify::new());
    let watcher = Watcher::new(
      vec![BundlerConfig::new(
        BundlerOptions {
          cwd: Some(cwd),
          input: Some(vec![input.to_string_lossy().into_owned().into()]),
          ..Default::default()
        },
        vec![Arc::new(FailingWatchChangePlugin {
          watch_change_calls: Arc::clone(&watch_change_calls),
          close_watcher_calls: Arc::clone(&close_watcher_calls),
        })],
      )],
      RecordingHandler { end: Arc::clone(&end), close_calls: Arc::clone(&handler_close_calls) },
      &WatcherConfig::default(),
    )
    .expect("create watcher");
    watcher.run().expect("start watcher");
    tokio::time::timeout(Duration::from_secs(10), end.notified())
      .await
      .expect("initial build should finish");

    watcher
      .tx
      .unbounded_send(WatcherMsg::FileChanges {
        task_index: WatchTaskIdx::from_usize(0),
        changes: vec![FileChangeEvent::new(
          input.to_string_lossy().into_owned(),
          WatcherChangeKind::Update,
        )],
      })
      .expect("send file change");
    tokio::time::timeout(Duration::from_secs(10), watcher.wait_for_close())
      .await
      .expect("watchChange failure should terminate the coordinator");

    let first_error = watcher.close().await.expect_err("watchChange failure should fail close");
    let first_message = first_error.to_string();
    assert!(first_message.contains("watch task 0 watchChange failed"));
    assert!(first_message.contains("intentional watchChange failure"));
    assert_eq!(watch_change_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 1);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);

    let replayed =
      watcher.close().await.expect_err("later close should replay watchChange failure");
    assert_eq!(replayed.to_string(), first_message);
    assert_eq!(watch_change_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_watcher_calls.load(Ordering::SeqCst), 1);
    assert_eq!(handler_close_calls.load(Ordering::SeqCst), 1);
  }
}
