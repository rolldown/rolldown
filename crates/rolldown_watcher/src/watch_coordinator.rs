use crate::event::WatchEvent;
use crate::file_change_event::FileChangeEvent;
use crate::handler::WatcherEventHandler;
use crate::watch_task::{BuildOutcome, WatchTask, WatchTaskIdx};
use crate::watcher::WatcherConfig;
use crate::watcher_msg::WatcherMsg;
use crate::watcher_state::WatcherState;
use event_listener::Event;
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt, pin_mut, select_biased};
use oxc_index::IndexVec;
use rolldown_common::WatcherChangeKind;
use rolldown_utils::indexmap::FxIndexMap;
use std::future::Future;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

enum DebounceWaitResult {
  Message(Option<WatcherMsg>),
  Timeout,
}

async fn wait_for_debounce_input(
  rx: &mut mpsc::UnboundedReceiver<WatcherMsg>,
  timeout: impl Future<Output = ()>,
) -> DebounceWaitResult {
  let timeout = timeout.fuse();
  pin_mut!(timeout);
  // Biased: a message already queued wins over a due deadline, so every part of
  // an OS notification that is already in the channel joins this build.
  select_biased! {
    message = rx.next() => DebounceWaitResult::Message(message),
    () = timeout => DebounceWaitResult::Timeout,
  }
}

/// The coordinator actor that owns all state and runs the event loop.
pub struct WatchCoordinator<H: WatcherEventHandler> {
  rx: mpsc::UnboundedReceiver<WatcherMsg>,
  handler: H,
  state: WatcherState,
  debounce_duration: Duration,
  tasks: IndexVec<WatchTaskIdx, WatchTask>,
  closed: Arc<AtomicBool>,
  close_notify: Arc<Event>,
}

impl<H: WatcherEventHandler> WatchCoordinator<H> {
  pub(crate) fn new(
    rx: mpsc::UnboundedReceiver<WatcherMsg>,
    handler: H,
    tasks: IndexVec<WatchTaskIdx, WatchTask>,
    config: &WatcherConfig,
    closed: Arc<AtomicBool>,
    close_notify: Arc<Event>,
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
          let msg = self.rx.next().await;
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
          let timeout = rolldown_utils::time::sleep_until(*deadline);

          match wait_for_debounce_input(&mut self.rx, timeout).await {
            DebounceWaitResult::Timeout => {
              let (new_state, changes) = mem::take(&mut self.state).on_debounce_timeout();
              self.state = new_state;

              if let Some(changes) = changes {
                if !self.run_build_sequence(changes).await {
                  self.handle_close().await;
                  break;
                }
              }
            }
            DebounceWaitResult::Message(message) => match message {
              Some(WatcherMsg::FileChanges { task_index, changes }) => {
                self.process_file_changes(task_index, changes).await;
              }
              Some(WatcherMsg::Close) => {
                self.handle_close().await;
                break;
              }
              None => break,
            },
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
    // Listen before checking `closed`: `event_listener::Event` stores no permit,
    // so a listener created after the notify would wait forever.
    let wait_for_close = async {
      let listener = self.close_notify.listen();
      if !self.closed.load(Ordering::Relaxed) {
        listener.await;
      }
    }
    .fuse();
    let handler = handler.fuse();
    pin_mut!(wait_for_close, handler);

    // Biased: close wins over the handler.
    select_biased! {
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
      // Buffered messages are still handed out after close, so drain until
      // `Err(_)` — which covers both "empty but open" and "closed and drained".
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::task_fs_event_handler::TaskFsEventHandler;
  use crate::watcher::WatcherConfig;
  use rolldown::{BundlerConfig, BundlerOptions};
  use rolldown_common::{OnInvalidate, WatchOption};
  use rolldown_fs_watcher::FsWatcherConfig;
  use rolldown_workspace::TestDir;
  use std::sync::Mutex;
  use std::sync::atomic::AtomicUsize;

  #[test]
  fn queued_message_wins_over_due_deadline() {
    futures::executor::block_on(async {
      let (tx, mut rx) = mpsc::unbounded();
      // An unbiased select would pick either ready branch at random; repeat so such a pick shows.
      for _ in 0..64 {
        tx.unbounded_send(WatcherMsg::Close).unwrap();
        let result = wait_for_debounce_input(&mut rx, std::future::ready(())).await;
        assert!(matches!(result, DebounceWaitResult::Message(Some(WatcherMsg::Close))));
      }
    });
  }

  /// After the initial build, queues three `FileChanges` messages for the entry back to back,
  /// the way the parts of one OS notification reach the coordinator. Closes the watcher at the
  /// end of the next build.
  struct Recorder {
    tx: mpsc::UnboundedSender<WatcherMsg>,
    invalidations: Arc<AtomicUsize>,
    /// How many messages the coordinator had taken when each rebuild started.
    taken_at_restart: Arc<Mutex<Vec<usize>>>,
  }

  impl Recorder {
    fn rebuilt(&self) -> bool {
      !self.taken_at_restart.lock().unwrap().is_empty()
    }
  }

  impl WatcherEventHandler for Recorder {
    async fn on_event(&self, event: WatchEvent) {
      match event {
        WatchEvent::BundleEnd(data) if !self.rebuilt() => {
          let watched: Vec<String> =
            data.bundle_handle.watch_files().iter().map(|file| file.to_string()).collect();
          let [entry] = watched.as_slice() else {
            panic!("expected one watched file: {watched:?}")
          };
          for _ in 0..3 {
            let changes = vec![FileChangeEvent::new(entry.clone(), WatcherChangeKind::Update)];
            let task_index = WatchTaskIdx::from_usize(0);
            self.tx.unbounded_send(WatcherMsg::FileChanges { task_index, changes }).unwrap();
          }
        }
        WatchEvent::End if self.rebuilt() => {
          self.tx.unbounded_send(WatcherMsg::Close).unwrap();
        }
        WatchEvent::Error(data) => panic!("build failed: {data:?}"),
        _ => {}
      }
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {}

    async fn on_restart(&self) {
      self.taken_at_restart.lock().unwrap().push(self.invalidations.load(Ordering::SeqCst));
    }

    async fn on_close(&self) {}
  }

  #[test]
  fn zero_build_delay_builds_queued_changes_together() {
    let dir = TestDir::new("rolldown-watcher-zero-delay");
    let entry = dir.path().join("main.js");
    std::fs::write(&entry, "export const value = 1;\n").unwrap();

    // Each message the coordinator takes calls `onInvalidate` once.
    let invalidations = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&invalidations);
    let on_invalidate = OnInvalidate::new(Arc::new(move |_path: &str| {
      counter.fetch_add(1, Ordering::SeqCst);
    }));
    let bundler_config = BundlerConfig::new(
      BundlerOptions {
        cwd: Some(dir.path().to_path_buf()),
        input: Some(vec![entry.to_string_lossy().into_owned().into()]),
        watch: Some(WatchOption { on_invalidate: Some(on_invalidate), ..Default::default() }),
        ..Default::default()
      },
      vec![],
    );

    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    // Fs watching is off: only the messages the recorder queues reach the coordinator.
    let task = WatchTask::new(
      bundler_config,
      TaskFsEventHandler { task_index: WatchTaskIdx::from_usize(0), tx: tx.clone() },
      &FsWatcherConfig { enabled: false, ..Default::default() },
      &closed,
    )
    .unwrap_or_else(|errors| panic!("create watch task: {errors:?}"));
    let mut tasks = IndexVec::new();
    tasks.push(task);

    let taken_at_restart = Arc::new(Mutex::new(Vec::new()));
    let recorder = Recorder { tx, invalidations, taken_at_restart: Arc::clone(&taken_at_restart) };
    let coordinator = WatchCoordinator::new(
      rx,
      recorder,
      tasks,
      &WatcherConfig { debounce: Some(Duration::ZERO), ..Default::default() },
      closed,
      Arc::new(Event::new()),
    );
    rolldown_utils::futures::block_on(coordinator.run());

    // One rebuild, started only after all three messages were taken.
    assert_eq!(*taken_at_restart.lock().unwrap(), vec![3]);
  }
}
