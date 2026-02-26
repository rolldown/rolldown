use crate::handler::WatcherEventHandler;
use crate::task_fs_event_handler::TaskFsEventHandler;
use crate::watch_coordinator::WatchCoordinator;
use crate::watch_task::{WatchTask, WatchTaskIdx};
use crate::watcher_msg::WatcherMsg;
use anyhow::Result;
use oxc_index::IndexVec;
use rolldown::BundlerConfig;
use rolldown_error::BuildResult;
use rolldown_fs_watcher::{FsWatcher, FsWatcherConfig};
#[cfg(not(target_family = "wasm"))]
use rolldown_fs_watcher::{PollFsWatcher, RecommendedFsWatcher};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, mpsc, oneshot};

/// Default debounce duration in milliseconds.
/// Matches Rollup's default buildDelay of 0ms.
const DEFAULT_DEBOUNCE_MS: u64 = 0;

/// Configuration for the watcher
#[derive(Debug, Clone, Default)]
pub struct WatcherConfig {
  /// Debounce duration for file changes
  pub debounce: Option<Duration>,
  /// Whether to use polling-based file watching instead of native OS events
  pub use_polling: bool,
  /// Poll interval in milliseconds (only used when `use_polling` is true)
  pub poll_interval: Option<u64>,
}

impl WatcherConfig {
  pub fn debounce_duration(&self) -> Duration {
    self.debounce.unwrap_or(Duration::from_millis(DEFAULT_DEBOUNCE_MS))
  }

  fn to_fs_watcher_config(&self) -> FsWatcherConfig {
    let mut config = FsWatcherConfig::default();
    if let Some(poll_interval) = self.poll_interval {
      config.poll_interval = poll_interval;
    }
    config
  }
}

/// The main watcher that manages multiple bundlers
pub struct Watcher {
  tx: mpsc::UnboundedSender<WatcherMsg>,
  task_handle: tokio::task::JoinHandle<()>,
  closed_notify: Arc<Notify>,
}

impl Drop for Watcher {
  fn drop(&mut self) {
    self.task_handle.abort();
  }
}

impl Watcher {
  /// Create a new watcher with a single bundler config
  pub fn new<H: WatcherEventHandler + 'static>(
    config: BundlerConfig,
    handler: H,
    watcher_config: &WatcherConfig,
  ) -> BuildResult<Self> {
    Self::with_multiple_bundler_configs(vec![config], handler, watcher_config)
  }

  /// Create a new watcher with multiple bundler configs
  pub fn with_multiple_bundler_configs<H: WatcherEventHandler + 'static>(
    configs: Vec<BundlerConfig>,
    handler: H,
    watcher_config: &WatcherConfig,
  ) -> BuildResult<Self> {
    let (tx, rx) = mpsc::unbounded_channel::<WatcherMsg>();

    let fs_watcher_config = watcher_config.to_fs_watcher_config();

    // Create per-task watch tasks with their own fs watchers
    let mut tasks = IndexVec::with_capacity(configs.len());
    for (index, config) in configs.into_iter().enumerate() {
      let task_index = WatchTaskIdx::from_usize(index);
      let fs_handler = TaskFsEventHandler { task_index, tx: tx.clone() };
      #[cfg(not(target_family = "wasm"))]
      let fs_watcher: Box<dyn FsWatcher + Send + 'static> = if watcher_config.use_polling {
        Box::new(PollFsWatcher::with_config(fs_handler, fs_watcher_config.clone())?)
      } else {
        Box::new(RecommendedFsWatcher::with_config(fs_handler, fs_watcher_config.clone())?)
      };
      #[cfg(target_family = "wasm")]
      let fs_watcher: Box<dyn FsWatcher + Send + 'static> = Box::new(
        rolldown_fs_watcher::NoopFsWatcher::with_config(fs_handler, fs_watcher_config.clone())?,
      );
      let task = WatchTask::new(config, fs_watcher)?;
      tasks.push(task);
    }

    let coordinator = WatchCoordinator::new(rx, handler, tasks, watcher_config);
    let closed_notify = Arc::new(Notify::new());
    let notify_clone = Arc::clone(&closed_notify);
    let task_handle = tokio::spawn(async move {
      coordinator.run().await;
      notify_clone.notify_waiters();
    });

    Ok(Self { tx, task_handle, closed_notify })
  }

  /// Get the closed notification handle.
  /// Useful for NAPI bindings where the lock can't be held across await points.
  pub fn closed_notify(&self) -> Arc<Notify> {
    Arc::clone(&self.closed_notify)
  }

  /// Wait until the watcher coordinator finishes (i.e., after close).
  /// On NAPI side, the pending Promise keeps Node.js event loop alive.
  pub async fn wait_for_close(&self) {
    self.closed_notify.notified().await;
  }

  /// Close the watcher
  pub async fn close(&self) -> Result<()> {
    let (response_tx, response_rx) = oneshot::channel();
    self
      .tx
      .send(WatcherMsg::Close(response_tx))
      .map_err(|_| anyhow::anyhow!("Watcher event loop already closed"))?;
    response_rx.await.map_err(|_| anyhow::anyhow!("Watcher event loop terminated unexpectedly"))?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

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
