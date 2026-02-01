use crate::bundler_task::BundlerTask;
use crate::emitter::{SharedWatcherEmitter, WatcherEmitter};
use crate::event::{BundleEvent, WatcherChangeData, WatcherEvent};
use crate::state::{ChangeEntry, WatcherState};
use anyhow::Result;
use itertools::Itertools;
use notify::{
  Config, RecommendedWatcher, Watcher as _,
  event::{ModifyKind, RenameMode},
};
use rolldown::BundlerConfig;
use rolldown_common::{NotifyOption, WatcherChangeKind};
use rolldown_error::BuildResult;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};

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
}

/// Message sent to the watcher event loop
enum WatcherMessage {
  /// File system event from notify
  FsEvent(notify::Result<notify::Event>),
  /// Request to close the watcher
  Close(oneshot::Sender<()>),
}

/// The main watcher that manages multiple bundlers
pub struct Watcher {
  /// Shared event emitter
  emitter: SharedWatcherEmitter,
  /// Channel to send messages to the event loop
  tx: mpsc::UnboundedSender<WatcherMessage>,
  /// Handle to the event loop task
  #[expect(dead_code)]
  task_handle: tokio::task::JoinHandle<()>,
}

impl Watcher {
  /// Create a new watcher with a single config
  pub fn new(config: BundlerConfig, watcher_config: &WatcherConfig) -> BuildResult<Self> {
    Self::with_configs(vec![config], watcher_config)
  }

  /// Create a new watcher with multiple configs
  pub fn with_configs(
    configs: Vec<BundlerConfig>,
    watcher_config: &WatcherConfig,
  ) -> BuildResult<Self> {
    let emitter = Arc::new(WatcherEmitter::new());

    // Create the notify watcher
    let (tx, mut rx) = mpsc::unbounded_channel::<WatcherMessage>();
    let tx_clone = tx.clone();

    let watch_option = {
      let mut config = Config::default();
      if let Some(notify) = &watcher_config.notify {
        if let Some(poll_interval) = notify.poll_interval {
          config = config.with_poll_interval(poll_interval);
        }
        config = config.with_compare_contents(notify.compare_contents);
      }
      config
    };

    let notify_watcher = RecommendedWatcher::new(
      move |res| {
        if let Err(e) = tx_clone.send(WatcherMessage::FsEvent(res)) {
          eprintln!(
            "Watcher: failed to send file change notification - channel closed while processing file system event: {e:?}"
          );
        }
      },
      watch_option,
    )
    .map_err(|e| anyhow::anyhow!("Failed to create notify watcher: {e}"))?;

    let notify_watcher = Arc::new(Mutex::new(notify_watcher));

    // Create bundler tasks
    let mut tasks = Vec::with_capacity(configs.len());
    for (index, config) in configs.into_iter().enumerate() {
      let task =
        BundlerTask::new(index, config, Arc::clone(&emitter), Arc::clone(&notify_watcher))?;
      tasks.push(task);
    }

    // Get debounce duration
    let debounce_duration = watcher_config.debounce_duration();

    // Start the event loop
    let emitter_clone = Arc::clone(&emitter);
    let task_handle = tokio::spawn(async move {
      run_event_loop(tasks, emitter_clone, &mut rx, debounce_duration).await;
    });

    Ok(Self { emitter, tx, task_handle })
  }

  /// Get the event emitter for subscribing to events
  pub fn emitter(&self) -> &SharedWatcherEmitter {
    &self.emitter
  }

  /// Close the watcher
  pub async fn close(&self) -> Result<()> {
    let (response_tx, response_rx) = oneshot::channel();
    self
      .tx
      .send(WatcherMessage::Close(response_tx))
      .map_err(|_| anyhow::anyhow!("Watcher event loop already closed"))?;
    response_rx.await.map_err(|_| anyhow::anyhow!("Watcher event loop terminated unexpectedly"))?;
    Ok(())
  }
}

/// The main event loop for the watcher
async fn run_event_loop(
  tasks: Vec<BundlerTask>,
  emitter: SharedWatcherEmitter,
  rx: &mut mpsc::UnboundedReceiver<WatcherMessage>,
  debounce_duration: Duration,
) {
  let mut state = WatcherState::Idle;

  // Perform initial build
  let _ = run_build(&tasks, &emitter).await;

  loop {
    match &state {
      WatcherState::Idle => {
        // Wait for file change or close
        let msg = rx.recv().await;
        match msg {
          Some(WatcherMessage::FsEvent(event)) => {
            if let Some(entries) = process_fs_event(event, &emitter) {
              for entry in entries {
                state = state.on_file_change(entry, debounce_duration);
              }
            }
          }
          Some(WatcherMessage::Close(response)) => {
            let _ = handle_close(state, &tasks, &emitter).await;
            let _ = response.send(());
            break;
          }
          None => {
            // Channel closed, exit
            break;
          }
        }
      }
      WatcherState::Debouncing { deadline, .. } => {
        let timeout = tokio::time::sleep_until((*deadline).into());

        tokio::select! {
          () = timeout => {
            let (new_state, changes) = state.on_debounce_timeout();
            state = new_state;

            if let Some(changes) = changes {
              // Notify plugins and invalidate bundlers
              for change in &changes {
                for task in &tasks {
                  task.on_change(change.path.as_str(), change.kind);
                  task.invalidate(change.path.as_str());
                }
              }

              // Deduplicate changed files for logging
              let _changed_files: Vec<&str> =
                changes.iter().map(|c| c.path.as_str()).unique().collect();

              let _ = run_build(&tasks, &emitter).await;

              // Transition state after build
              state = state.on_build_complete(debounce_duration);
            }
          }
          msg = rx.recv() => {
            match msg {
              Some(WatcherMessage::FsEvent(event)) => {
                if let Some(entries) = process_fs_event(event, &emitter) {
                  for entry in entries {
                    state = state.on_file_change(entry, debounce_duration);
                  }
                }
              }
              Some(WatcherMessage::Close(response)) => {
                let _ = handle_close(state, &tasks, &emitter).await;
                let _ = response.send(());
                break;
              }
              None => {
                break;
              }
            }
          }
        }
      }
      WatcherState::Building { .. } => {
        // This shouldn't happen in our design since we handle builds synchronously
        // But if it does, just process messages
        let msg = rx.recv().await;
        match msg {
          Some(WatcherMessage::FsEvent(event)) => {
            if let Some(entries) = process_fs_event(event, &emitter) {
              for entry in entries {
                state = state.on_file_change(entry, debounce_duration);
              }
            }
          }
          Some(WatcherMessage::Close(response)) => {
            let _ = handle_close(state, &tasks, &emitter).await;
            let _ = response.send(());
            break;
          }
          None => {
            break;
          }
        }
      }
      WatcherState::Closing | WatcherState::Closed => {
        break;
      }
    }
  }
}

/// Process a file system event and return change entries if any
fn process_fs_event(
  event: notify::Result<notify::Event>,
  emitter: &SharedWatcherEmitter,
) -> Option<Vec<ChangeEntry>> {
  match event {
    Ok(event) => {
      tracing::debug!(name = "notify event", event = ?event);
      let mut entries = Vec::new();

      for path in event.paths {
        let id = path.to_string_lossy();
        let kind = match event.kind {
          notify::EventKind::Create(_) => Some(WatcherChangeKind::Create),
          notify::EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any) => {
            tracing::debug!(name = "notify updated content", path = ?id.as_ref());
            Some(WatcherChangeKind::Update)
          }
          notify::EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            tracing::debug!(name = "notify renamed file", path = ?id.as_ref());
            Some(WatcherChangeKind::Update)
          }
          notify::EventKind::Remove(_) => Some(WatcherChangeKind::Delete),
          _ => None,
        };

        if let Some(kind) = kind {
          // Emit change event
          emitter.emit(WatcherEvent::Change(WatcherChangeData::new(id.clone().into(), kind)));
          entries.push(ChangeEntry::new(id.into(), kind));
        }
      }

      if entries.is_empty() { None } else { Some(entries) }
    }
    Err(e) => {
      eprintln!("notify error: {e:?}");
      None
    }
  }
}

/// Run a build across all bundler tasks
async fn run_build(tasks: &[BundlerTask], emitter: &SharedWatcherEmitter) -> BuildResult<()> {
  emitter.emit(WatcherEvent::Event(BundleEvent::Start));

  for task in tasks {
    task.build().await?;
  }

  emitter.emit(WatcherEvent::Event(BundleEvent::End));

  Ok(())
}

/// Handle the close request
async fn handle_close(
  state: WatcherState,
  tasks: &[BundlerTask],
  emitter: &SharedWatcherEmitter,
) -> WatcherState {
  let (new_state, should_close) = state.on_close();
  if should_close {
    // Close all bundler tasks
    for task in tasks {
      if let Err(e) = task.close().await {
        eprintln!("Error closing bundler task: {e:?}");
      }
    }

    // Emit close event
    emitter.emit(WatcherEvent::Close);
  }
  new_state.to_closed()
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
}
