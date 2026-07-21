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
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
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
  // `select_biased!` requires each branch future to be `Unpin + FusedFuture`.
  // `rx.next()` (StreamExt::Next) already is, so it is used inline. The custom
  // `Sleep` only impls `Future`, so fuse it (for `FusedFuture`) and pin it (for
  // `Unpin`). The biased order matches tokio's `biased;`: message first, timeout
  // second, so a queued change at the deadline still extends the debounce window.
  let timeout = timeout.fuse();
  pin_mut!(timeout);
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
          // Runtime-aware timer facade: the async-runtime build has no tokio
          // reactor, so `tokio::time::sleep_until` would panic here ("no
          // reactor running"). Every rx arm below drops this future when it
          // wins the select -- the facade's Sleep cancels on drop, matching
          // tokio's semantics, so the deadline-extension loop is unchanged.
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
  /// 5. For each task needing rebuild: BundleStart → build → BundleEnd/Error,
  ///    then drain queued messages and repeat while any task needs a rebuild
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

    // Step 5: Build each task that needs it.
    //
    // A single filesystem save can fan out into multiple per-task
    // `FileChanges` messages: every task's fs watcher reports the same path
    // independently. The shared runtime's timer fires a 0ms debounce deadline
    // immediately, where tokio's timer wheel rounded it up ~1ms — long enough
    // for the sibling messages to win the debounce select and coalesce into
    // one rebuild. Dispatching `End` while a sibling message is already queued
    // would split one save into two rebuild envelopes, so before `End` the
    // queue is drained and the build pass repeats while any task still needs a
    // rebuild, keeping the whole save in a single Start..End envelope.
    let mut dispatched_changes = changes;
    let mut rebuilt_tasks: IndexVec<WatchTaskIdx, bool> =
      self.tasks.iter().map(|_| false).collect();
    loop {
      for task_index in self.tasks.indices() {
        if !self.tasks[task_index].needs_rebuild {
          continue;
        }

        let task = &self.tasks[task_index];
        if !self.dispatch_event(WatchEvent::BundleStart(task.start_event_data(task_index))).await {
          return false;
        }

        let task = &mut self.tasks[task_index];
        let outcome = task.build(task_index).await;
        // This task has now been rebuilt inside this envelope: any message it
        // reports from here on cannot be its own report of the save that
        // scheduled the rebuild — it is a genuinely new filesystem event.
        rebuilt_tasks[task_index] = true;
        match outcome {
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

      // Pull the messages queued while the builds ran, remembering which paths
      // were reported by a task that already rebuilt in this envelope: such a
      // message cannot be a same-save sibling report — that task's report of
      // the save was consumed before its build ran — so those paths are new
      // saves that must be re-reported below even when their (path, kind) was
      // already dispatched.
      let mut new_save_paths = FxIndexSet::default();
      self
        .drain_buffered_events_observing(|task_index, changes| {
          if rebuilt_tasks[task_index] {
            new_save_paths.extend(changes.iter().map(|change| change.path.clone()));
          }
        })
        .await;
      if matches!(self.state, WatcherState::Closing | WatcherState::Closed) {
        // A queued close was drained; the close sequence has already run.
        return true;
      }
      if !self.tasks.iter().any(|task| task.needs_rebuild) {
        break;
      }

      // Consume the drained changes so they don't schedule a duplicate
      // envelope after `End`. Same-save sibling messages repeat a (path, kind)
      // this envelope already reported and stay silent; a genuinely new save
      // of an already-reported path re-dispatches `change`/`watchChange`.
      let (new_state, drained_changes) = mem::take(&mut self.state).on_debounce_timeout();
      self.state = new_state;
      if let Some(drained_changes) = drained_changes {
        for (path, kind) in drained_changes {
          if !new_save_paths.contains(&path) && dispatched_changes.get(&path).copied() == Some(kind)
          {
            continue;
          }
          if !self.dispatch_change(path.as_str(), kind).await {
            return false;
          }
          for task in &self.tasks {
            task.call_watch_change(path.as_str(), kind).await;
          }
          dispatched_changes.insert(path, kind);
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
    // Listen-before-check idiom for `event_listener::Event` (no stored permit):
    // create the listener before reading `closed`. `publish_close` sets `closed`
    // before `notify`, so a listener created here before the notify is woken, and
    // observing `closed == true` skips the permit-less await.
    let wait_for_close = async {
      let listener = self.close_notify.listen();
      if !self.closed.load(Ordering::Relaxed) {
        listener.await;
      }
    }
    .fuse();
    // Consumer callbacks only impl `Future`, so fuse the handler too and pin
    // both for `select_biased!` (needs `Unpin + FusedFuture`).
    let handler = handler.fuse();
    pin_mut!(wait_for_close, handler);

    // Biased order matches tokio's `biased;`: close first, then handler.
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
    self.drain_buffered_events_observing(|_, _| {}).await;
  }

  /// Like [`Self::drain_buffered_events`], but lets the caller observe each
  /// drained `FileChanges` message with its task provenance, which the state
  /// machine's path-keyed merge discards.
  async fn drain_buffered_events_observing(
    &mut self,
    mut observe: impl FnMut(WatchTaskIdx, &[FileChangeEvent]),
  ) {
    loop {
      // `futures`' non-deprecated `try_recv()` mirrors tokio's `try_recv()` shape
      // exactly: `Ok(msg)` = a buffered message (still drained after close while
      // any remain), `Err(TryRecvError::Empty)` = empty but open, and
      // `Err(TryRecvError::Closed)` = closed and fully drained. tokio mapped both
      // its `Empty` and `Disconnected` errors to the same break, so the single
      // `Err(_)` arm preserves the original semantics unchanged.
      match self.rx.try_recv() {
        Ok(WatcherMsg::FileChanges { task_index, changes }) => {
          observe(task_index, &changes);
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
  use rolldown::{BundlerConfig, BundlerOptions};
  use rolldown_error::BuildResult;
  use rolldown_fs_watcher::{
    DynFsWatcher, FsEventHandler, FsWatcher, FsWatcherConfig, PathsMut, RecursiveMode,
  };
  use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    sync::atomic::AtomicUsize,
  };
  // `event_listener::Event` backs the coordinator's production `close_notify`
  // field. `tokio::sync::Notify` is kept ONLY for the tests' internal signals.
  use tokio::sync::Notify;

  static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

  struct TestDir(PathBuf);

  impl TestDir {
    fn new() -> Self {
      let path = std::env::temp_dir().join(format!(
        "rolldown-watch-coordinator-{}-{}",
        std::process::id(),
        NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
      ));
      fs::create_dir_all(&path).expect("create test directory");
      Self(path)
    }
  }

  impl Drop for TestDir {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }

  struct NoopWatcher;
  struct NoopPaths;

  impl PathsMut for NoopPaths {
    fn add(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
      Ok(())
    }

    fn remove(&mut self, _path: &Path) -> BuildResult<()> {
      Ok(())
    }

    fn commit(self: Box<Self>) -> BuildResult<()> {
      Ok(())
    }
  }

  impl FsWatcher for NoopWatcher {
    fn new<F: FsEventHandler>(_event_handler: F) -> BuildResult<Self>
    where
      Self: Sized,
    {
      Ok(Self)
    }

    fn with_config<F: FsEventHandler>(
      _event_handler: F,
      _config: FsWatcherConfig,
    ) -> BuildResult<Self>
    where
      Self: Sized,
    {
      Ok(Self)
    }

    fn watch(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
      Ok(())
    }

    fn unwatch(&mut self, _path: &Path) -> BuildResult<()> {
      Ok(())
    }

    fn paths_mut<'me>(&'me mut self) -> Box<dyn PathsMut + 'me> {
      Box::new(NoopPaths)
    }
  }

  /// Simulates one filesystem save fanning out into two per-task
  /// `FileChanges` messages: the sibling task's message is queued during the
  /// rebuild's BundleEnd dispatch, i.e. strictly before the coordinator can
  /// dispatch `End`.
  struct SameSaveInjectingHandler {
    events: Arc<Mutex<Vec<String>>>,
    tx: mpsc::UnboundedSender<WatcherMsg>,
    inject_path: String,
    inject_task_index: WatchTaskIdx,
    injected: AtomicBool,
    end_count: Arc<AtomicUsize>,
    initial_end: Arc<Notify>,
    rebuild_end: Arc<Notify>,
  }

  impl WatcherEventHandler for SameSaveInjectingHandler {
    async fn on_event(&self, event: WatchEvent) {
      if matches!(event, WatchEvent::BundleEnd(_))
        && self.end_count.load(Ordering::SeqCst) == 1
        && !self.injected.swap(true, Ordering::SeqCst)
      {
        self
          .tx
          .unbounded_send(WatcherMsg::FileChanges {
            task_index: self.inject_task_index,
            changes: vec![FileChangeEvent::new(
              self.inject_path.clone(),
              WatcherChangeKind::Update,
            )],
          })
          .expect("queue sibling file change");
      }

      self.events.lock().expect("events lock").push(event.as_str().to_string());

      if matches!(event, WatchEvent::End) {
        let ends = self.end_count.fetch_add(1, Ordering::SeqCst) + 1;
        if ends == 1 {
          self.initial_end.notify_one();
        } else {
          self.rebuild_end.notify_one();
        }
      }
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {
      self.events.lock().expect("events lock").push("CHANGE".to_string());
    }

    async fn on_restart(&self) {
      self.events.lock().expect("events lock").push("RESTART".to_string());
    }

    async fn on_close(&self) {
      self.events.lock().expect("events lock").push("CLOSE".to_string());
    }
  }

  #[tokio::test]
  async fn queued_change_wins_when_debounce_timeout_is_already_ready() {
    let (tx, mut rx) = mpsc::unbounded();
    tx.unbounded_send(WatcherMsg::FileChanges {
      task_index: WatchTaskIdx::from_usize(0),
      changes: vec![FileChangeEvent::new("main.js".to_string(), WatcherChangeKind::Update)],
    })
    .expect("queue file change");

    let result = wait_for_debounce_input(&mut rx, std::future::ready(())).await;

    assert!(matches!(result, DebounceWaitResult::Message(Some(WatcherMsg::FileChanges { .. }))));
  }

  /// Counts `watchChange` plugin hook invocations.
  #[derive(Debug)]
  struct WatchChangeCountingPlugin {
    count: Arc<AtomicUsize>,
  }

  impl rolldown::plugin::Plugin for WatchChangeCountingPlugin {
    fn name(&self) -> std::borrow::Cow<'static, str> {
      "watch-change-counter".into()
    }

    fn register_hook_usage(&self) -> rolldown::plugin::HookUsage {
      rolldown::plugin::HookUsage::WatchChange
    }

    async fn watch_change(
      &self,
      _ctx: &rolldown::plugin::PluginContext,
      _path: &str,
      _event: WatcherChangeKind,
    ) -> rolldown::plugin::HookNoopReturn {
      self.count.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  /// Simulates a genuinely new save of the same file (same change kind) that
  /// lands while the previous save's rebuild is still inside its envelope: the
  /// second save's message is queued during the rebuild's BundleEnd dispatch,
  /// strictly before the coordinator can dispatch `End`.
  struct SecondSaveInjectingHandler {
    events: Arc<Mutex<Vec<String>>>,
    tx: mpsc::UnboundedSender<WatcherMsg>,
    inject_path: String,
    injected: AtomicBool,
    end_count: Arc<AtomicUsize>,
    initial_end: Arc<Notify>,
    rebuild_end: Arc<Notify>,
  }

  impl WatcherEventHandler for SecondSaveInjectingHandler {
    async fn on_event(&self, event: WatchEvent) {
      if matches!(event, WatchEvent::BundleEnd(_))
        && self.end_count.load(Ordering::SeqCst) == 1
        && !self.injected.swap(true, Ordering::SeqCst)
      {
        // The first save's rebuild just finished building; the user saves the
        // file again (same path, same kind) before `End` is dispatched.
        self
          .tx
          .unbounded_send(WatcherMsg::FileChanges {
            task_index: WatchTaskIdx::from_usize(0),
            changes: vec![FileChangeEvent::new(
              self.inject_path.clone(),
              WatcherChangeKind::Update,
            )],
          })
          .expect("queue second save");
      }

      self.events.lock().expect("events lock").push(event.as_str().to_string());

      if matches!(event, WatchEvent::End) {
        let ends = self.end_count.fetch_add(1, Ordering::SeqCst) + 1;
        if ends == 1 {
          self.initial_end.notify_one();
        } else {
          self.rebuild_end.notify_one();
        }
      }
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) {
      self.events.lock().expect("events lock").push("CHANGE".to_string());
    }

    async fn on_restart(&self) {
      self.events.lock().expect("events lock").push("RESTART".to_string());
    }

    async fn on_close(&self) {
      self.events.lock().expect("events lock").push("CLOSE".to_string());
    }
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn second_save_during_rebuild_still_reports_change_and_watch_change() {
    let test_dir = TestDir::new();
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write input");
    let input = fs::canonicalize(input).expect("canonicalize input");
    let cwd = input.parent().expect("input has parent").to_path_buf();
    let input_str = input.to_string_lossy().into_owned();

    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());
    let watch_change_count = Arc::new(AtomicUsize::new(0));

    let mut tasks = IndexVec::new();
    let fs_watcher: DynFsWatcher = Box::new(NoopWatcher);
    let task = WatchTask::new(
      BundlerConfig::new(
        BundlerOptions {
          cwd: Some(cwd),
          input: Some(vec![input_str.clone().into()]),
          file: Some("dist/out.js".into()),
          ..Default::default()
        },
        vec![Arc::new(WatchChangeCountingPlugin { count: Arc::clone(&watch_change_count) })],
      ),
      fs_watcher,
      &closed,
    )
    .expect("create watch task");
    tasks.push(task);

    let events = Arc::new(Mutex::new(Vec::new()));
    let end_count = Arc::new(AtomicUsize::new(0));
    let initial_end = Arc::new(Notify::new());
    let rebuild_end = Arc::new(Notify::new());
    let coordinator = WatchCoordinator::new(
      rx,
      SecondSaveInjectingHandler {
        events: Arc::clone(&events),
        tx: tx.clone(),
        inject_path: input_str.clone(),
        injected: AtomicBool::new(false),
        end_count: Arc::clone(&end_count),
        initial_end: Arc::clone(&initial_end),
        rebuild_end: Arc::clone(&rebuild_end),
      },
      tasks,
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(30), initial_end.notified())
      .await
      .expect("initial build should finish");

    // Save #1. Save #2 is queued by the handler while save #1's rebuild
    // BundleEnd is being dispatched.
    tx.unbounded_send(WatcherMsg::FileChanges {
      task_index: WatchTaskIdx::from_usize(0),
      changes: vec![FileChangeEvent::new(input_str.clone(), WatcherChangeKind::Update)],
    })
    .expect("send first file change");

    tokio::time::timeout(Duration::from_secs(30), rebuild_end.notified())
      .await
      .expect("rebuild should finish");

    closed.store(true, Ordering::Relaxed);
    close_notify.notify(usize::MAX);
    tx.unbounded_send(WatcherMsg::Close).expect("send close");
    tokio::time::timeout(Duration::from_secs(30), handle)
      .await
      .expect("coordinator should close")
      .expect("coordinator task should not panic");

    let events = events.lock().expect("events lock").clone();
    // Probe validation: save #1's notifications must be observable at all.
    assert!(
      events.iter().filter(|e| *e == "CHANGE").count() >= 1,
      "save #1 should report a change event; events: {events:?}"
    );
    assert!(watch_change_count.load(Ordering::SeqCst) >= 1, "save #1 should invoke watchChange");
    // The genuinely new second save must be reported too: one more `change`
    // event and one more `watchChange` invocation, still in one envelope.
    assert_eq!(
      events,
      [
        // Initial build.
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        // One rebuild envelope covering both saves: save #2 lands after save
        // #1's build pass, is reported, and triggers the second build pass.
        "CHANGE",
        "RESTART",
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "CHANGE",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        "CLOSE",
      ]
    );
    assert_eq!(
      watch_change_count.load(Ordering::SeqCst),
      2,
      "each distinct save must invoke the watchChange hook"
    );
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn same_save_sibling_message_stays_in_one_rebuild_envelope() {
    let test_dir = TestDir::new();
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write input");
    let input = fs::canonicalize(input).expect("canonicalize input");
    let cwd = input.parent().expect("input has parent").to_path_buf();
    let input_str = input.to_string_lossy().into_owned();

    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());

    let mut tasks = IndexVec::new();
    for out_file in ["dist0/out.js", "dist1/out.js"] {
      let fs_watcher: DynFsWatcher = Box::new(NoopWatcher);
      let task = WatchTask::new(
        BundlerConfig::new(
          BundlerOptions {
            cwd: Some(cwd.clone()),
            input: Some(vec![input_str.clone().into()]),
            file: Some(out_file.into()),
            ..Default::default()
          },
          vec![],
        ),
        fs_watcher,
        &closed,
      )
      .expect("create watch task");
      tasks.push(task);
    }

    let events = Arc::new(Mutex::new(Vec::new()));
    let end_count = Arc::new(AtomicUsize::new(0));
    let initial_end = Arc::new(Notify::new());
    let rebuild_end = Arc::new(Notify::new());
    let coordinator = WatchCoordinator::new(
      rx,
      SameSaveInjectingHandler {
        events: Arc::clone(&events),
        tx: tx.clone(),
        inject_path: input_str.clone(),
        inject_task_index: WatchTaskIdx::from_usize(1),
        injected: AtomicBool::new(false),
        end_count: Arc::clone(&end_count),
        initial_end: Arc::clone(&initial_end),
        rebuild_end: Arc::clone(&rebuild_end),
      },
      tasks,
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(30), initial_end.notified())
      .await
      .expect("initial build should finish");

    // The first per-task message of the save. Its sibling for task 1 is queued
    // by the handler while task 0's rebuild BundleEnd is being dispatched.
    tx.unbounded_send(WatcherMsg::FileChanges {
      task_index: WatchTaskIdx::from_usize(0),
      changes: vec![FileChangeEvent::new(input_str.clone(), WatcherChangeKind::Update)],
    })
    .expect("send first file change");

    tokio::time::timeout(Duration::from_secs(30), rebuild_end.notified())
      .await
      .expect("rebuild should finish");

    closed.store(true, Ordering::Relaxed);
    close_notify.notify(usize::MAX);
    tx.unbounded_send(WatcherMsg::Close).expect("send close");
    tokio::time::timeout(Duration::from_secs(30), handle)
      .await
      .expect("coordinator should close")
      .expect("coordinator task should not panic");

    assert_eq!(
      *events.lock().expect("events lock"),
      [
        // Initial build: both tasks in one envelope.
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        // One rebuild envelope for the whole save: the drained sibling message
        // neither re-dispatches CHANGE nor splits the envelope.
        "CHANGE",
        "RESTART",
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        "CLOSE",
      ]
    );
    assert_eq!(end_count.load(Ordering::SeqCst), 2);
  }
}
