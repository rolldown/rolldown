use crate::handler::WatcherEventHandler;
use crate::msg::WatcherMsg;
use crate::watch_coordinator::WatchCoordinator;
use crate::watch_task::{WatchTask, WatchTaskIdx};
use anyhow::Result;
use oxc_index::IndexVec;
use rolldown::BundlerConfig;
use rolldown_common::NotifyOption;
use rolldown_error::BuildResult;
use rolldown_fs_watcher::{
  FsEventHandler, FsEventResult, FsWatcher, FsWatcherConfig, RecommendedFsWatcher,
};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// Default debounce duration in milliseconds
const DEFAULT_DEBOUNCE_MS: u64 = 100;

/// Configuration for the watcher
#[derive(Debug, Clone, Default)]
pub struct WatcherConfig {
  /// Debounce duration for file changes
  pub debounce: Option<Duration>,
  /// Notify (file system watcher) options
  pub notify: Option<NotifyOption>,
}

impl WatcherConfig {
  pub fn debounce_duration(&self) -> Duration {
    self.debounce.unwrap_or(Duration::from_millis(DEFAULT_DEBOUNCE_MS))
  }

  #[expect(clippy::cast_possible_truncation)]
  fn to_fs_watcher_config(&self) -> FsWatcherConfig {
    let mut config = FsWatcherConfig::default();
    if let Some(notify) = &self.notify {
      if let Some(poll_interval) = notify.poll_interval {
        config.poll_interval = poll_interval.as_millis() as u64;
      }
      config.compare_contents_for_polling = notify.compare_contents;
    }
    config
  }
}

/// Bridge that forwards fs events from a per-task watcher to the shared mpsc channel.
struct TaskFsEventHandler {
  task_index: WatchTaskIdx,
  tx: mpsc::UnboundedSender<WatcherMsg>,
}

impl FsEventHandler for TaskFsEventHandler {
  fn handle_event(&mut self, event: FsEventResult) {
    let _ = self.tx.send(WatcherMsg::FsEvent { task_index: self.task_index, event });
  }
}

/// The main watcher that manages multiple bundlers
pub struct Watcher {
  tx: mpsc::UnboundedSender<WatcherMsg>,
  task_handle: tokio::task::JoinHandle<()>,
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
      let fs_watcher: Box<dyn FsWatcher + Send + 'static> =
        Box::new(RecommendedFsWatcher::with_config(fs_handler, fs_watcher_config.clone())?);
      let task = WatchTask::new(config, fs_watcher)?;
      tasks.push(task);
    }

    let coordinator = WatchCoordinator::new(rx, handler, tasks, watcher_config);
    let task_handle = tokio::spawn(async move {
      coordinator.run().await;
    });

    Ok(Self { tx, task_handle })
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
    let config = WatcherConfig { debounce: Some(Duration::from_millis(500)), notify: None };
    assert_eq!(config.debounce_duration(), Duration::from_millis(500));
  }

  #[test]
  fn test_fs_watcher_config_defaults_when_notify_is_none() {
    let config = WatcherConfig { debounce: None, notify: None };
    let fs_config = config.to_fs_watcher_config();
    assert_eq!(fs_config.poll_interval, 100);
    assert!(!fs_config.compare_contents_for_polling);
  }

  #[test]
  fn test_fs_watcher_config_with_poll_interval() {
    let config = WatcherConfig {
      debounce: None,
      notify: Some(NotifyOption {
        poll_interval: Some(Duration::from_millis(250)),
        compare_contents: false,
      }),
    };
    let fs_config = config.to_fs_watcher_config();
    assert_eq!(fs_config.poll_interval, 250);
    assert!(!fs_config.compare_contents_for_polling);
  }

  #[test]
  fn test_fs_watcher_config_with_compare_contents() {
    let config = WatcherConfig {
      debounce: None,
      notify: Some(NotifyOption { poll_interval: None, compare_contents: true }),
    };
    let fs_config = config.to_fs_watcher_config();
    assert_eq!(fs_config.poll_interval, 100);
    assert!(fs_config.compare_contents_for_polling);
  }
}
