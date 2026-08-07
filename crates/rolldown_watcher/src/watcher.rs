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
    let tasks = Self::create_tasks(configs, watcher_config, &tx, &closed)?;
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
    self.signal_close();
    self.wait_for_close().await;
    Ok(())
  }

  /// The synchronous part of `close`: tell the coordinator to stop, without awaiting it. `Close`
  /// drives the idle and debouncing states; `closed` + `close_notify` also interrupt an in-flight
  /// user callback.
  fn signal_close(&self) {
    self.closed.store(true, std::sync::atomic::Ordering::Relaxed);
    self.close_notify.notify_one();
    let _ = self.tx.send(WatcherMsg::Close);
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

impl Drop for Watcher {
  // The coordinator keeps a `tx` clone per task, so its channel never closes on its own. Signal it
  // here, or a watcher dropped without `close` would leak the spawned coordinator task.
  fn drop(&mut self) {
    self.signal_close();
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

  /// Regression: a `run()` watcher dropped without `close()` must still stop its spawned
  /// coordinator. Each task's fs watcher holds a `tx.clone()`, so the coordinator's channel never
  /// closes on its own; `on_close` firing after the drop proves `Drop` signalled it, not leaked.
  #[tokio::test(flavor = "multi_thread")]
  async fn dropping_a_running_watcher_shuts_down_the_coordinator() {
    use crate::{WatchEvent, WatcherEventHandler};
    use rolldown::{BundlerConfig, BundlerOptions};
    use rolldown_common::{WatchOption, WatcherChangeKind};
    use std::sync::Arc;
    use sugar_path::SugarPath as _;
    use tokio::sync::Notify;

    struct SignalHandler {
      built: Arc<Notify>,
      closed: Arc<Notify>,
    }

    impl WatcherEventHandler for SignalHandler {
      async fn on_event(&self, event: WatchEvent) {
        if matches!(event, WatchEvent::End) {
          self.built.notify_one();
        }
      }
      async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {}
      async fn on_restart(&self) {}
      async fn on_close(&self) {
        self.closed.notify_one();
      }
    }

    let built = Arc::new(Notify::new());
    let closed = Arc::new(Notify::new());

    let cwd =
      rolldown_workspace::crate_dir("rolldown").join("./examples/basic").normalize().into_owned();
    let config = BundlerConfig::new(
      BundlerOptions {
        input: Some(vec!["./entry.js".to_string().into()]),
        cwd: Some(cwd),
        // Keep the build in-memory so the test never touches disk.
        watch: Some(WatchOption { skip_write: true, ..Default::default() }),
        ..Default::default()
      },
      vec![],
    );

    let handler = SignalHandler { built: Arc::clone(&built), closed: Arc::clone(&closed) };
    let watcher =
      Watcher::new(vec![config], handler, &WatcherConfig::default()).expect("failed to create");
    watcher.run();

    // Wait for the initial build so the coordinator is parked in `Idle` on `rx.recv()`.
    tokio::time::timeout(Duration::from_secs(30), built.notified())
      .await
      .expect("initial build did not finish");

    // Drop without `close()`; `Drop` must signal the coordinator to shut down.
    drop(watcher);

    tokio::time::timeout(Duration::from_secs(30), closed.notified())
      .await
      .expect("coordinator did not shut down after drop");
  }
}
