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
}

impl WatcherConfig {
  pub fn debounce_duration(&self) -> std::time::Duration {
    self.debounce.unwrap_or(std::time::Duration::from_millis(DEFAULT_DEBOUNCE_MS))
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
    let tasks = Self::create_tasks(configs, &tx, &closed)?;
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

  /// Spawn the coordinator. Only the first call starts it; a coordinator the
  /// runtime refused to start is dropped with the error.
  pub fn run(&self) -> Result<(), WatcherStartError> {
    let mut state = self.coordinator_state.lock().unwrap();
    let Some(coordinator) = state.coordinator.take() else {
      return Ok(());
    };
    let join_handle = try_spawn(coordinator).map_err(|(error, _)| WatcherStartError::new(error))?;
    let handle: PendingCoordinatorFuture = Box::pin(async move {
      let _ = join_handle.await;
    });
    state.handle = Some(handle.shared());
    Ok(())
  }

  /// Gives consumers a reliable way to await the watcher's completion.
  pub async fn wait_for_close(&self) {
    let handle = self.coordinator_state.lock().unwrap().handle.clone();
    if let Some(handle) = handle {
      handle.await;
    }
  }

  /// Mark the watcher closed and wake the coordinator. Only the first call
  /// publishes; it neither starts nor awaits the coordinator.
  fn publish_close(&self) {
    if self.closed.swap(true, std::sync::atomic::Ordering::Relaxed) {
      return;
    }
    // Wake the coordinator even when it is waiting for a user event callback. The mpsc message
    // remains the normal state-machine input when the coordinator is idle or debouncing.
    // `event_listener::Event` stores no permit, so this notify must come after the `closed`
    // store above; waiters listen before checking the flag to close the race.
    self.close_notify.notify(usize::MAX);
    let _ = self.tx.unbounded_send(WatcherMsg::Close);
  }

  /// Close the watcher and wait for the coordinator to finish.
  pub async fn close(&self) -> Result<()> {
    // Publish before starting a not-yet-run coordinator: otherwise a pool worker
    // could enter the initial build before it sees the close signal.
    self.publish_close();
    self.run().map_err(anyhow::Error::new)?;
    self.wait_for_close().await;
    Ok(())
  }

  fn create_tasks(
    configs: Vec<BundlerConfig>,
    tx: &mpsc::UnboundedSender<WatcherMsg>,
    closed: &Arc<AtomicBool>,
  ) -> BuildResult<IndexVec<WatchTaskIdx, WatchTask>> {
    let fs_watcher_config = FsWatcherConfig::default();
    let mut tasks = IndexVec::with_capacity(configs.len());
    for (index, config) in configs.into_iter().enumerate() {
      let task_index = WatchTaskIdx::from_usize(index);
      let fs_handler = TaskFsEventHandler { task_index, tx: tx.clone() };
      let task = WatchTask::new(config, fs_handler, &fs_watcher_config, closed)?;
      tasks.push(task);
    }
    Ok(tasks)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn test_watcher_config_default_debounce() {
    let config = WatcherConfig::default();
    assert_eq!(config.debounce_duration(), Duration::from_millis(DEFAULT_DEBOUNCE_MS));
  }

  #[test]
  fn test_watcher_config_custom_debounce() {
    let config = WatcherConfig { debounce: Some(Duration::from_millis(500)) };
    assert_eq!(config.debounce_duration(), Duration::from_millis(500));
  }
}
