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
#[cfg(target_family = "wasm")]
use rolldown_fs_watcher::FsWatcher;
use rolldown_fs_watcher::FsWatcherConfig;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc;

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
    // rolldown_watcher doesn't support debounce currently
    config.use_debounce = false;
    config
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
    let tasks = Self::create_tasks(configs, watcher_config, &tx)?;
    let coordinator = WatchCoordinator::new(rx, handler, tasks, watcher_config);
    let coordinator_future: Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(coordinator.run());

    Ok(Self {
      coordinator_state: std::sync::Mutex::new(CoordinatorState {
        coordinator: Some(coordinator_future),
        handle: None,
      }),
      tx,
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
    let _ = self.tx.send(WatcherMsg::Close);
    self.wait_for_close().await;
    Ok(())
  }

  fn create_tasks(
    configs: Vec<BundlerConfig>,
    watcher_config: &WatcherConfig,
    tx: &mpsc::UnboundedSender<WatcherMsg>,
  ) -> BuildResult<IndexVec<WatchTaskIdx, WatchTask>> {
    let fs_watcher_config = watcher_config.to_fs_watcher_config();
    let mut tasks = IndexVec::with_capacity(configs.len());
    for (index, config) in configs.into_iter().enumerate() {
      let task_index = WatchTaskIdx::from_usize(index);
      let fs_handler = TaskFsEventHandler { task_index, tx: tx.clone() };
      #[cfg(not(target_family = "wasm"))]
      let fs_watcher =
        rolldown_fs_watcher::create_fs_watcher(fs_handler, fs_watcher_config.clone())?;
      #[cfg(target_family = "wasm")]
      let fs_watcher: Box<dyn FsWatcher + Send + 'static> = Box::new(
        rolldown_fs_watcher::NoopFsWatcher::with_config(fs_handler, fs_watcher_config.clone())?,
      );
      let task = WatchTask::new(config, fs_watcher)?;
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
}
