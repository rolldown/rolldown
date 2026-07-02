use crate::event::WatchEvent;
use crate::file_change_event::FileChangeEvent;
use crate::handler::WatcherEventHandler;
use crate::watch_task::{BuildOutcome, WatchTask, WatchTaskIdx};
use crate::watcher::WatcherConfig;
use crate::watcher_msg::WatcherMsg;
use crate::watcher_state::WatcherState;
use oxc_index::IndexVec;
use rolldown_common::WatcherChangeKind;
use rolldown_utils::indexmap::FxIndexMap;
use std::future::Future;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, mpsc};

/// The coordinator actor that owns all state and runs the event loop.
pub struct WatchCoordinator<H: WatcherEventHandler> {
  rx: mpsc::UnboundedReceiver<WatcherMsg>,
  handler: H,
  state: WatcherState,
  debounce_duration: Duration,
  tasks: IndexVec<WatchTaskIdx, WatchTask>,
  closed: Arc<AtomicBool>,
  close_notify: Arc<Notify>,
}

impl<H: WatcherEventHandler> WatchCoordinator<H> {
  pub(crate) fn new(
    rx: mpsc::UnboundedReceiver<WatcherMsg>,
    handler: H,
    tasks: IndexVec<WatchTaskIdx, WatchTask>,
    config: &WatcherConfig,
    closed: Arc<AtomicBool>,
    close_notify: Arc<Notify>,
  ) -> Self {
    Self {
      rx,
      handler,
      state: WatcherState::Idle,
      debounce_duration: config.debounce_duration(),
      tasks,
      closed,
      close_notify,
    }
  }

  /// Main event loop: initial build → loop on state
  pub(crate) async fn run(mut self) {
    // Perform initial build
    if !self.run_initial_build().await {
      self.handle_close().await;
      return;
    }

    loop {
      match &self.state {
        WatcherState::Idle => {
          let msg = self.rx.recv().await;
          match msg {
            Some(WatcherMsg::FileChanges { task_index, changes }) => {
              self.process_file_changes(task_index, changes).await;
            }
            Some(WatcherMsg::Close) => {
              self.handle_close().await;
              break;
            }
            None => break,
          }
        }
        WatcherState::Debouncing { deadline, .. } => {
          // Runtime-aware timer facade: the async-runtime build has no tokio
          // reactor, so `tokio::time::sleep_until` would panic here ("no
          // reactor running"). Every rx arm below drops this future when it
          // wins the select -- the facade's Sleep cancels on drop, matching
          // tokio's semantics, so the deadline-extension loop is unchanged.
          let timeout = rolldown_utils::time::sleep_until(*deadline);

          tokio::select! {
            () = timeout => {
              let (new_state, changes) = mem::take(&mut self.state).on_debounce_timeout();
              self.state = new_state;

              if let Some(changes) = changes {
                if !self.run_build_sequence(changes).await {
                  self.handle_close().await;
                  break;
                }
              }
            }
            msg = self.rx.recv() => {
              match msg {
                Some(WatcherMsg::FileChanges { task_index, changes }) => {
                  self.process_file_changes(task_index, changes).await;
                }
                Some(WatcherMsg::Close) => {
                  self.handle_close().await;
                  break;
                }
                None => break,
              }
            }
          }
        }
        WatcherState::Closing | WatcherState::Closed => {
          break;
        }
      }
    }
  }

  /// Run the initial build for all tasks
  async fn run_initial_build(&mut self) -> bool {
    if !self.dispatch_event(WatchEvent::Start).await {
      return false;
    }

    for task_index in self.tasks.indices() {
      let task = &self.tasks[task_index];
      if !self.dispatch_event(WatchEvent::BundleStart(task.start_event_data(task_index))).await {
        return false;
      }

      let task = &mut self.tasks[task_index];
      match task.build(task_index).await {
        Ok(BuildOutcome::Success(data)) => {
          if !self.dispatch_event(WatchEvent::BundleEnd(data)).await {
            return false;
          }
        }
        Ok(BuildOutcome::Error(data)) => {
          if !self.dispatch_event(WatchEvent::Error(data)).await {
            return false;
          }
        }
        Ok(BuildOutcome::Skipped) => {}
        Ok(BuildOutcome::Closed) => return false,
        Err(errs) => {
          let error_messages: Vec<String> =
            errs.iter().map(|e| e.to_diagnostic().to_string()).collect();
          tracing::error!("Fatal build error: {error_messages:?}");
        }
      }
    }

    self.dispatch_event(WatchEvent::End).await
  }

  /// The rebuild sequence matching Rollup's semantics (spec §2.8):
  /// 1. handler.on_change for each changed file
  /// 2. For each task and each changed file: task.call_watch_change
  /// 3. handler.on_restart
  /// 4. handler.on_event(Start)
  /// 5. For each task needing rebuild: BundleStart → build → BundleEnd/Error
  /// 6. handler.on_event(End)
  /// 7. drain_buffered_events
  async fn run_build_sequence(&mut self, changes: FxIndexMap<String, WatcherChangeKind>) -> bool {
    // Step 1 & 2: Notify handler and plugin hooks for each change
    for (path, kind) in &changes {
      if !self.dispatch_change(path.as_str(), *kind).await {
        return false;
      }
    }

    for task in &self.tasks {
      for (path, kind) in &changes {
        task.call_watch_change(path.as_str(), *kind).await;
      }
    }

    // Step 3: Restart notification
    if !self.dispatch_restart().await {
      return false;
    }

    // Step 4: Start event
    if !self.dispatch_event(WatchEvent::Start).await {
      return false;
    }

    // Step 5: Build each task that needs it
    for task_index in self.tasks.indices() {
      if !self.tasks[task_index].needs_rebuild {
        continue;
      }

      let task = &self.tasks[task_index];
      if !self.dispatch_event(WatchEvent::BundleStart(task.start_event_data(task_index))).await {
        return false;
      }

      let task = &mut self.tasks[task_index];
      match task.build(task_index).await {
        Ok(BuildOutcome::Success(data)) => {
          if !self.dispatch_event(WatchEvent::BundleEnd(data)).await {
            return false;
          }
        }
        Ok(BuildOutcome::Error(data)) => {
          if !self.dispatch_event(WatchEvent::Error(data)).await {
            return false;
          }
        }
        Ok(BuildOutcome::Skipped) => {}
        Ok(BuildOutcome::Closed) => return false,
        Err(errs) => {
          let error_messages: Vec<String> =
            errs.iter().map(|e| e.to_diagnostic().to_string()).collect();
          tracing::error!("Fatal build error: {error_messages:?}");
        }
      }
    }

    // Step 6: End event
    if !self.dispatch_event(WatchEvent::End).await {
      return false;
    }

    // Step 7: Drain buffered events that arrived during the build
    self.drain_buffered_events().await;
    true
  }

  async fn dispatch_event(&self, event: WatchEvent) -> bool {
    self.await_handler_or_close(self.handler.on_event(event)).await
  }

  async fn dispatch_change(&self, path: &str, kind: WatcherChangeKind) -> bool {
    self.await_handler_or_close(self.handler.on_change(path, kind)).await
  }

  async fn dispatch_restart(&self) -> bool {
    self.await_handler_or_close(self.handler.on_restart()).await
  }

  /// Await a consumer event callback while keeping close re-entrant.
  ///
  /// A callback may call and await `watcher.close()`. Waiting only for the callback would deadlock:
  /// close waits for this coordinator, while the coordinator waits for the callback. On close, drop
  /// only the Rust-side wait for the callback; the JavaScript promise keeps running, and the
  /// coordinator performs the complete close sequence before `watcher.close()` resolves.
  async fn await_handler_or_close<F>(&self, handler: F) -> bool
  where
    F: Future<Output = ()>,
  {
    let wait_for_close = async {
      if !self.closed.load(Ordering::Relaxed) {
        self.close_notify.notified().await;
      }
    };

    tokio::select! {
      biased;
      () = wait_for_close => false,
      () = handler => !self.closed.load(Ordering::Relaxed),
    }
  }

  /// Process file changes: call on_invalidate per file, mark task for rebuild,
  /// then batch all changes into a single state transition.
  async fn process_file_changes(
    &mut self,
    task_index: WatchTaskIdx,
    changes: Vec<FileChangeEvent>,
  ) {
    let mut effective_changes: Vec<FileChangeEvent> = Vec::new();

    if let Some(task) = self.tasks.get_mut(task_index) {
      for change in changes {
        if task.mark_needs_rebuild(&change.path) {
          task.call_on_invalidate(&change.path).await;
          effective_changes.push(change);
        }
      }
    }

    if effective_changes.is_empty() {
      return;
    }

    self.state =
      mem::take(&mut self.state).on_file_changes(effective_changes, self.debounce_duration);
  }

  /// Drain buffered fs events that arrived during a build.
  /// Uses try_recv to process all pending messages without blocking.
  async fn drain_buffered_events(&mut self) {
    loop {
      match self.rx.try_recv() {
        Ok(WatcherMsg::FileChanges { task_index, changes }) => {
          self.process_file_changes(task_index, changes).await;
        }
        Ok(WatcherMsg::Close) => {
          self.handle_close().await;
          return;
        }
        Err(_) => break,
      }
    }
  }

  /// Handle close: call close_watcher hooks, close bundlers, emit close
  async fn handle_close(&mut self) {
    let (new_state, should_close) = mem::take(&mut self.state).on_close();
    self.state = new_state;

    if should_close {
      // Close watcher hooks on all tasks
      for task in &self.tasks {
        task.call_hook_close_watcher().await;
      }

      // Close all bundlers
      for task in &self.tasks {
        if let Err(e) = task.close().await {
          tracing::error!("Error closing bundler: {e:?}");
        }
      }

      self.handler.on_close().await;
    }

    self.state = mem::take(&mut self.state).to_closed();
  }
}
