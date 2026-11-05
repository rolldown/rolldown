use crate::handler::WatcherEventHandler;
use crate::task_fs_event_handler::TaskFsEventHandler;
use crate::watch_coordinator::WatchCoordinator;
use crate::watch_task::{WatchTask, WatchTaskIdx};
use crate::watcher_msg::WatcherMsg;
use anyhow::Result;
use futures::FutureExt;
use futures::future::Shared;
use oxc_index::IndexVec;
use rolldown::BundlerConfig;
use rolldown_error::BuildResult;
use rolldown_fs_watcher::FsWatcherConfig;
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
}

impl WatcherConfig {
  pub fn debounce_duration(&self) -> std::time::Duration {
    self.debounce.unwrap_or(std::time::Duration::from_millis(DEFAULT_DEBOUNCE_MS))
  }
}

type CoordinatorFuture = Shared<Pin<Box<dyn Future<Output = ()> + Send>>>;

struct CoordinatorState {
  /// The coordinator future, before `run()` is called.
  coordinator: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
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
    let tasks = Self::create_tasks(configs, &tx, &closed)?;
    let coordinator = WatchCoordinator::new(
      rx,
      handler,
      tasks,
      watcher_config,
      Arc::clone(&closed),
      Arc::clone(&close_notify),
    );
    let coordinator_future: Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(coordinator.run());

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
      let join_handle = tokio::spawn(coordinator);
      let handle: Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(async move {
        let _ = join_handle.await;
      });
      state.handle = Some(handle.shared());
    }
  }

  /// Gives consumers a reliable way to await the watcher's completion.
  pub async fn wait_for_close(&self) {
    let handle = self.coordinator_state.lock().unwrap().handle.clone();
    if let Some(handle) = handle {
      handle.await;
    }
  }

  /// Close the watcher and wait for the coordinator to finish.
  /// Must be called after `run()` — calling before `run()` will skip cleanup hooks.
  pub async fn close(&self) -> Result<()> {
    self.closed.store(true, std::sync::atomic::Ordering::Relaxed);
    // Wake the coordinator even when it is waiting for a user event callback. The mpsc message
    // remains the normal state-machine input when the coordinator is idle or debouncing.
    self.close_notify.notify_one();
    let _ = self.tx.send(WatcherMsg::Close);
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
