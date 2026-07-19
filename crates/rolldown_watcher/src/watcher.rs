use crate::handler::WatcherEventHandler;
use crate::task_fs_event_handler::TaskFsEventHandler;
use crate::watch_coordinator::WatchCoordinator;
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

type PendingCoordinatorFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
type CoordinatorFuture = Shared<PendingCoordinatorFuture>;

struct CoordinatorState {
  /// The coordinator future, before `run()` is called.
  coordinator: Option<PendingCoordinatorFuture>,
  /// The spawned handle, after `run()` is called. Shared so multiple callers can await.
  handle: Option<CoordinatorFuture>,
}

impl CoordinatorState {
  /// Accepted and completed starts are idempotent; a rejected start hands the
  /// coordinator future back so a later `run()` can retry the submission after
  /// the runtime restarts.
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
    let tasks = Self::create_tasks(configs, watcher_config, &tx, &closed)?;
    let coordinator = WatchCoordinator::new(
      rx,
      handler,
      tasks,
      watcher_config,
      Arc::clone(&closed),
      Arc::clone(&close_notify),
    );
    let coordinator_future: PendingCoordinatorFuture = Box::pin(coordinator.run());

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

  /// Spawn the coordinator. Accepted and completed starts are idempotent; a
  /// runtime-rejected start retains the coordinator for a later retry.
  pub fn run(&self) -> Result<(), WatcherStartError> {
    self.start_coordinator(|coordinator| match try_spawn(coordinator) {
      Ok(join_handle) => {
        let handle: PendingCoordinatorFuture = Box::pin(async move {
          let _ = join_handle.await;
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
      handle.await;
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
    self.wait_for_close().await;
    Ok(())
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
  use std::{
    convert::Infallible,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
  };

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
    handle.await;
    let state = watcher.coordinator_state.lock().unwrap();
    assert!(state.coordinator.is_none());
    assert_eq!(accepted_submissions.load(Ordering::SeqCst), 1);
    assert_eq!(runs.load(Ordering::SeqCst), 1);
  }
}
