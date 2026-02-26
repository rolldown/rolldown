use crate::event::WatchEvent;
use crate::handler::WatcherEventHandler;
use crate::msg::WatcherMsg;
use crate::state::{ChangeEntry, WatcherState};
use crate::watch_task::{BuildOutcome, WatchTask, WatchTaskIdx};
use crate::watcher::WatcherConfig;
use oxc_index::IndexVec;
use rolldown_common::WatcherChangeKind;
use rolldown_fs_watcher::FsEventResult;
use std::mem;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// The coordinator actor that owns all state and runs the event loop.
pub struct WatchCoordinator<H: WatcherEventHandler> {
  rx: mpsc::UnboundedReceiver<WatcherMsg>,
  handler: H,
  state: WatcherState,
  debounce_duration: Duration,
  tasks: IndexVec<WatchTaskIdx, WatchTask>,
}

impl<H: WatcherEventHandler> WatchCoordinator<H> {
  pub(crate) fn new(
    rx: mpsc::UnboundedReceiver<WatcherMsg>,
    handler: H,
    tasks: IndexVec<WatchTaskIdx, WatchTask>,
    config: &WatcherConfig,
  ) -> Self {
    Self {
      rx,
      handler,
      state: WatcherState::Idle,
      debounce_duration: config.debounce_duration(),
      tasks,
    }
  }

  /// Main event loop: initial build → loop on state
  pub(crate) async fn run(mut self) {
    // Perform initial build
    self.run_initial_build().await;

    loop {
      match &self.state {
        WatcherState::Idle => {
          let msg = self.rx.recv().await;
          match msg {
            Some(WatcherMsg::FsEvent { task_index, event }) => {
              self.process_fs_event(task_index, event).await;
            }
            Some(WatcherMsg::Close(reply)) => {
              self.handle_close(reply).await;
              break;
            }
            None => break,
          }
        }
        WatcherState::Debouncing { deadline, .. } => {
          let timeout = tokio::time::sleep_until((*deadline).into());

          tokio::select! {
            () = timeout => {
              let (new_state, changes) = mem::take(&mut self.state).on_debounce_timeout();
              self.state = new_state;

              if let Some(changes) = changes {
                self.run_build_sequence(changes).await;
              }
            }
            msg = self.rx.recv() => {
              match msg {
                Some(WatcherMsg::FsEvent { task_index, event }) => {
                  self.process_fs_event(task_index, event).await;
                }
                Some(WatcherMsg::Close(reply)) => {
                  self.handle_close(reply).await;
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
  async fn run_initial_build(&mut self) {
    self.handler.on_event(WatchEvent::Start).await;

    for task_index in self.tasks.indices() {
      let task = &self.tasks[task_index];
      self.handler.on_event(WatchEvent::BundleStart(task.start_event_data(task_index))).await;

      let task = &mut self.tasks[task_index];
      match task.build(task_index).await {
        Ok(BuildOutcome::Success(data)) => {
          self.handler.on_event(WatchEvent::BundleEnd(data)).await;
        }
        Ok(BuildOutcome::Error(data)) => {
          self.handler.on_event(WatchEvent::Error(data)).await;
        }
        Ok(BuildOutcome::Skipped) => {}
        Err(errs) => {
          let error_messages: Vec<String> = errs.iter().map(|e| format!("{e:?}")).collect();
          tracing::error!("Fatal build error: {error_messages:?}");
        }
      }
    }

    self.handler.on_event(WatchEvent::End).await;
  }

  /// The rebuild sequence matching Rollup's semantics (spec §2.8):
  /// 1. handler.on_change for each changed file
  /// 2. For each task and each changed file: task.call_watch_change
  /// 3. handler.on_restart
  /// 4. handler.on_event(Start)
  /// 5. For each task needing rebuild: BundleStart → build → BundleEnd/Error
  /// 6. handler.on_event(End)
  /// 7. drain_buffered_events
  async fn run_build_sequence(&mut self, changes: Vec<ChangeEntry>) {
    // Step 1 & 2: Notify handler and plugin hooks for each change
    for change in &changes {
      self.handler.on_change(change.path.as_str(), change.kind).await;
    }

    for task in &self.tasks {
      for change in &changes {
        task.call_watch_change(change.path.as_str(), change.kind).await;
      }
    }

    // Step 3: Restart notification
    self.handler.on_restart().await;

    // Step 4: Start event
    self.handler.on_event(WatchEvent::Start).await;

    // Step 5: Build each task that needs it
    for task_index in self.tasks.indices() {
      if !self.tasks[task_index].needs_rebuild {
        continue;
      }

      let task = &self.tasks[task_index];
      self.handler.on_event(WatchEvent::BundleStart(task.start_event_data(task_index))).await;

      let task = &mut self.tasks[task_index];
      match task.build(task_index).await {
        Ok(BuildOutcome::Success(data)) => {
          self.handler.on_event(WatchEvent::BundleEnd(data)).await;
        }
        Ok(BuildOutcome::Error(data)) => {
          self.handler.on_event(WatchEvent::Error(data)).await;
        }
        Ok(BuildOutcome::Skipped) => {}
        Err(errs) => {
          let error_messages: Vec<String> = errs.iter().map(|e| format!("{e:?}")).collect();
          tracing::error!("Fatal build error: {error_messages:?}");
        }
      }
    }

    // Step 6: End event
    self.handler.on_event(WatchEvent::End).await;

    // Step 7: Drain buffered events that arrived during the build
    self.drain_buffered_events().await;
  }

  /// Process a file system event: map notify events to ChangeEntry,
  /// call on_invalidate, and update state.
  async fn process_fs_event(&mut self, task_index: WatchTaskIdx, event: FsEventResult) {
    match event {
      Ok(fs_events) => {
        for fs_event in fs_events {
          tracing::debug!(name = "notify event", event = ?fs_event.detail);

          for path in &fs_event.detail.paths {
            let id = path.to_string_lossy();
            let kind = match fs_event.detail.kind {
              notify::EventKind::Create(_) => Some(WatcherChangeKind::Create),
              notify::EventKind::Modify(
                notify::event::ModifyKind::Data(_) | notify::event::ModifyKind::Any,
              ) => {
                tracing::debug!(name = "notify updated content", path = ?id.as_ref());
                Some(WatcherChangeKind::Update)
              }
              notify::EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::To,
              )) => {
                tracing::debug!(name = "notify renamed file", path = ?id.as_ref());
                Some(WatcherChangeKind::Update)
              }
              notify::EventKind::Remove(_) => Some(WatcherChangeKind::Delete),
              _ => None,
            };

            if let Some(kind) = kind {
              // Call on_invalidate for the affected task
              if let Some(task) = self.tasks.get_mut(task_index) {
                task.invalidate(&id);
                task.call_on_invalidate(&id).await;
              }

              let entry = ChangeEntry::new(id.into(), kind);
              self.state = mem::take(&mut self.state).on_file_change(entry, self.debounce_duration);
            }
          }
        }
      }
      Err(errors) => {
        for e in errors {
          tracing::error!("notify error: {e:?}");
        }
      }
    }
  }

  /// Drain buffered fs events that arrived during a build.
  /// Uses try_recv to process all pending messages without blocking.
  async fn drain_buffered_events(&mut self) {
    loop {
      match self.rx.try_recv() {
        Ok(WatcherMsg::FsEvent { task_index, event }) => {
          self.process_fs_event(task_index, event).await;
        }
        Ok(WatcherMsg::Close(reply)) => {
          self.handle_close(reply).await;
          return;
        }
        Err(_) => break,
      }
    }
  }

  /// Handle close: call close_watcher hooks, close bundlers, emit close, reply
  async fn handle_close(&mut self, reply: oneshot::Sender<()>) {
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
    let _ = reply.send(());
  }
}
