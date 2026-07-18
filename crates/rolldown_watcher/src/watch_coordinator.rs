use crate::event::WatchEvent;
use crate::file_change_event::FileChangeEvent;
use crate::handler::WatcherEventHandler;
use crate::watch_task::{BuildOutcome, WatchTask, WatchTaskBuildError, WatchTaskIdx};
use crate::watcher::WatcherConfig;
use crate::watcher_msg::WatcherMsg;
use crate::watcher_state::WatcherState;
use event_listener::Event;
use futures::FutureExt;
use futures::channel::mpsc;
use futures::{StreamExt, pin_mut, select_biased};
use oxc_index::IndexVec;
use rolldown_common::WatcherChangeKind;
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic};
use rolldown_utils::indexmap::FxIndexMap;
use std::any::Any;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WATCH_REGISTRATION_RETRY_DELAYS: [Duration; 3] =
  [Duration::from_millis(25), Duration::from_millis(100), Duration::from_millis(250)];

pub type CoordinatorCloseResult = Result<(), Arc<CoordinatorCloseError>>;

#[derive(Debug)]
pub struct CoordinatorCloseError {
  message: Arc<str>,
  failures: Box<[CoordinatorCloseFailure]>,
}

#[derive(Debug)]
pub struct CoordinatorCloseFailure {
  message: Arc<str>,
  source: Option<CoordinatorCloseSource>,
}

#[derive(Debug)]
enum CoordinatorCloseSource {
  Anyhow(anyhow::Error),
  Diagnostic(BuildDiagnostic),
}

enum HandlerDispatchResult {
  Completed,
  CloseRequested,
  Failed(anyhow::Error),
}

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

impl CoordinatorCloseFailure {
  fn from_build_error(context: &str, error: BatchedBuildDiagnostic) -> Vec<Self> {
    let diagnostics = error.into_vec();
    if diagnostics.is_empty() {
      return vec![Self {
        message: Arc::from(format!("{context}: build failed without diagnostics")),
        source: None,
      }];
    }

    diagnostics
      .into_iter()
      .map(|diagnostic| Self {
        message: Arc::from(format!("{context}: {}", diagnostic.to_diagnostic())),
        source: Some(CoordinatorCloseSource::Diagnostic(diagnostic)),
      })
      .collect()
  }

  fn from_anyhow_error(context: &str, error: anyhow::Error) -> Vec<Self> {
    match error.downcast::<BatchedBuildDiagnostic>() {
      Ok(error) => Self::from_build_error(context, error),
      Err(error) => vec![Self {
        message: Arc::from(format!("{context}: {error:#}")),
        source: Some(CoordinatorCloseSource::Anyhow(error)),
      }],
    }
  }

  fn from_panic(context: &str, payload: Box<dyn Any + Send + 'static>) -> Self {
    let message = format!("{context}: {}", panic_payload_message(&*payload));
    discard_panic_payload(payload);
    Self { message: message.into(), source: None }
  }

  pub fn message(&self) -> &str {
    &self.message
  }
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> &str {
  if let Some(message) = payload.downcast_ref::<String>() {
    message
  } else if let Some(message) = payload.downcast_ref::<&str>() {
    message
  } else {
    "non-string panic payload"
  }
}

fn discard_panic_payload(payload: Box<dyn Any + Send + 'static>) {
  if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(payload)))
    && let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload)))
  {
    mem::forget(nested_payload);
  }
}

impl CoordinatorCloseError {
  fn from_errors(failures: Vec<CoordinatorCloseFailure>) -> Self {
    let message = Arc::from(format!(
      "Watcher close failed:\n- {}",
      failures.iter().map(CoordinatorCloseFailure::message).collect::<Vec<_>>().join("\n- ")
    ));
    Self { message, failures: failures.into_boxed_slice() }
  }

  pub(crate) fn from_message(message: impl Into<Arc<str>>) -> Self {
    let message = message.into();
    Self {
      message: Arc::clone(&message),
      failures: vec![CoordinatorCloseFailure { message, source: None }].into_boxed_slice(),
    }
  }

  pub fn failures(&self) -> &[CoordinatorCloseFailure] {
    &self.failures
  }
}

impl fmt::Display for CoordinatorCloseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.message.fmt(f)
  }
}

impl Error for CoordinatorCloseError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    (self.failures.len() == 1).then(|| self.failures[0].source()).flatten()
  }
}

impl fmt::Display for CoordinatorCloseFailure {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.message.fmt(f)
  }
}

impl Error for CoordinatorCloseFailure {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    self.source.as_ref().map(|source| match source {
      CoordinatorCloseSource::Anyhow(error) => error.root_cause(),
      CoordinatorCloseSource::Diagnostic(error) => error,
    })
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
  native_owned_close_identities: Arc<Mutex<Vec<u64>>>,
  close_error: Option<Arc<CoordinatorCloseError>>,
  event_loop_errors: Vec<CoordinatorCloseFailure>,
}

impl<H: WatcherEventHandler> WatchCoordinator<H> {
  pub(crate) fn new(
    rx: mpsc::UnboundedReceiver<WatcherMsg>,
    handler: H,
    tasks: IndexVec<WatchTaskIdx, WatchTask>,
    config: &WatcherConfig,
    closed: Arc<AtomicBool>,
    close_notify: Arc<Event>,
    native_owned_close_identities: Arc<Mutex<Vec<u64>>>,
  ) -> Self {
    Self {
      rx,
      handler,
      state: WatcherState::Idle,
      debounce_duration: config.debounce_duration(),
      tasks,
      closed,
      close_notify,
      native_owned_close_identities,
      close_error: None,
      event_loop_errors: Vec::new(),
    }
  }

  /// Run the event loop behind an unwind boundary, then perform cleanup exactly once.
  pub(crate) async fn run(mut self) -> CoordinatorCloseResult {
    let mut errors = Vec::new();

    match AssertUnwindSafe(self.run_loop()).catch_unwind().await {
      Ok(()) => {
        errors.append(&mut self.event_loop_errors);
      }
      Err(payload) => {
        errors.push(CoordinatorCloseFailure::from_panic(
          "watch coordinator event loop panicked",
          payload,
        ));
      }
    }

    // See internal-docs/watch-mode/implementation.md.
    self.handle_close(errors).await;
    self.close_result()
  }

  /// Main event loop: initial build → loop on state.
  async fn run_loop(&mut self) {
    // Perform initial build
    if !self.run_initial_build().await {
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
            Some(WatcherMsg::Close) | None => return,
          }
        }
        WatcherState::Debouncing { deadline, .. } => {
          // Runtime-aware timer facade: the async-runtime build has no tokio
          // reactor, so `tokio::time::sleep_until` would panic here ("no
          // reactor running"). Every rx arm below drops this future when it
          // wins the select -- the facade's Sleep cancels on drop, matching
          // tokio's semantics, so the deadline-extension loop is unchanged.
          let timeout = rolldown_utils::time::sleep_until(*deadline);

          // A queued file change observed at the deadline must extend the
          // debounce window instead of producing an avoidable intermediate build.
          // See internal-docs/watch-mode/implementation.md.
          match wait_for_debounce_input(&mut self.rx, timeout).await {
            DebounceWaitResult::Timeout => {
              let (new_state, changes) = mem::take(&mut self.state).on_debounce_timeout();
              self.state = new_state;

              if let Some(changes) = changes {
                if !self.run_build_sequence(changes).await {
                  return;
                }
              }
            }
            DebounceWaitResult::Message(message) => match message {
              Some(WatcherMsg::FileChanges { task_index, changes }) => {
                self.process_file_changes(task_index, changes).await;
              }
              Some(WatcherMsg::Close) | None => return,
            },
          }
        }
        WatcherState::Closing | WatcherState::Closed => {
          return;
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

      let Some(outcome) = self.build_task_with_registration_retries(task_index).await else {
        return false;
      };
      match outcome {
        BuildOutcome::Success(data) => {
          if !self.dispatch_event(WatchEvent::BundleEnd(data)).await {
            return false;
          }
        }
        BuildOutcome::Error(data) => {
          if !self.dispatch_event(WatchEvent::Error(data)).await {
            return false;
          }
        }
        BuildOutcome::Skipped => {}
        BuildOutcome::Closed => return false,
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

    for task_index in self.tasks.indices() {
      for (path, kind) in &changes {
        if let Err(error) = self.tasks[task_index].call_watch_change(path.as_str(), *kind).await {
          self.event_loop_errors.extend(CoordinatorCloseFailure::from_anyhow_error(
            &format!("watch task {} watchChange failed", task_index.index()),
            error,
          ));
          return false;
        }
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

      let Some(outcome) = self.build_task_with_registration_retries(task_index).await else {
        return false;
      };
      match outcome {
        BuildOutcome::Success(data) => {
          if !self.dispatch_event(WatchEvent::BundleEnd(data)).await {
            return false;
          }
        }
        BuildOutcome::Error(data) => {
          if !self.dispatch_event(WatchEvent::Error(data)).await {
            return false;
          }
        }
        BuildOutcome::Skipped => {}
        BuildOutcome::Closed => return false,
      }
    }

    // Step 6: End event
    if !self.dispatch_event(WatchEvent::End).await {
      return false;
    }

    // Step 7: Drain buffered events that arrived during the build
    self.drain_buffered_events().await
  }

  async fn build_task_with_registration_retries(
    &mut self,
    task_index: WatchTaskIdx,
  ) -> Option<BuildOutcome> {
    let mut retry_delays = WATCH_REGISTRATION_RETRY_DELAYS.iter().copied();

    loop {
      match self.tasks[task_index].build(task_index).await {
        Ok(outcome) => return Some(outcome),
        Err(WatchTaskBuildError::WatchRegistration { diagnostics, bundle_handle }) => {
          let Some(delay) = retry_delays.next() else {
            self.event_loop_errors.extend(CoordinatorCloseFailure::from_build_error(
              &format!(
                "watch task {} file watcher registration failed after {} retries",
                task_index.index(),
                WATCH_REGISTRATION_RETRY_DELAYS.len()
              ),
              diagnostics,
            ));
            return None;
          };

          tracing::warn!(
            task_index = task_index.index(),
            retry_delay_ms = delay.as_millis(),
            error = %diagnostics,
            "File watcher registration failed; retrying task build"
          );
          if let Err(error) = bundle_handle.close().await {
            tracing::error!(
              task_index = task_index.index(),
              error = %error,
              "Failed to close bundle from a failed watcher registration attempt"
            );
            // The normal coordinator close path retries this same idempotent
            // handle close and records its terminal error alongside registration.
            self.event_loop_errors.extend(CoordinatorCloseFailure::from_build_error(
              &format!(
                "watch task {} file watcher registration failed before retry cleanup completed",
                task_index.index()
              ),
              diagnostics,
            ));
            return None;
          }
          if !self.wait_for_registration_retry(delay).await {
            return None;
          }
        }
      }
    }
  }

  async fn wait_for_registration_retry(&mut self, delay: Duration) -> bool {
    let deadline = Instant::now() + delay;

    loop {
      let closed = Arc::clone(&self.closed);
      let close_notify = Arc::clone(&self.close_notify);
      // Listen-before-check idiom for `event_listener::Event`, which (unlike
      // tokio's `Notify`) stores no permit: create the listener first, then read
      // `closed`. `publish_close` sets `closed` before `notify`, so a listener
      // created before that notify is woken, and observing `closed == true` here
      // means the close already happened and we skip an await that could
      // otherwise miss the permit-less wake.
      let wait_for_close = async move {
        let listener = close_notify.listen();
        if !closed.load(Ordering::Relaxed) {
          listener.await;
        }
      }
      .fuse();
      let timeout = rolldown_utils::time::sleep_until(deadline).fuse();
      pin_mut!(wait_for_close, timeout);

      // The biased order matches tokio's `biased;` (close, message, timeout).
      // The message is extracted from the select and handled *after* the select
      // block ends, so the `&mut self.rx` borrow held by `rx.next()` is released
      // before `process_file_changes` reborrows `&mut self`. The close and
      // timeout arms only `return`, so they need no `self` access.
      let msg = select_biased! {
        () = wait_for_close => return false,
        msg = self.rx.next() => msg,
        () = timeout => return true,
      };
      match msg {
        Some(WatcherMsg::FileChanges { task_index, changes }) => {
          self.process_file_changes(task_index, changes).await;
        }
        Some(WatcherMsg::Close) | None => return false,
      }
    }
  }

  async fn dispatch_event(&mut self, event: WatchEvent) -> bool {
    let result = self.await_handler_or_close(self.handler.on_event(event)).await;
    self.finish_handler_dispatch("watch event listener failed", result)
  }

  async fn dispatch_change(&mut self, path: &str, kind: WatcherChangeKind) -> bool {
    let result = self.await_handler_or_close(self.handler.on_change(path, kind)).await;
    self.finish_handler_dispatch("watch change listener failed", result)
  }

  async fn dispatch_restart(&mut self) -> bool {
    let result = self.await_handler_or_close(self.handler.on_restart()).await;
    self.finish_handler_dispatch("watch restart listener failed", result)
  }

  fn finish_handler_dispatch(&mut self, context: &str, result: HandlerDispatchResult) -> bool {
    match result {
      HandlerDispatchResult::Completed => true,
      HandlerDispatchResult::CloseRequested => false,
      HandlerDispatchResult::Failed(error) => {
        self.event_loop_errors.extend(CoordinatorCloseFailure::from_anyhow_error(context, error));
        false
      }
    }
  }

  /// Await a consumer event callback while keeping close re-entrant.
  ///
  /// A callback may call and await `watcher.close()`. Waiting only for the callback would deadlock:
  /// close waits for this coordinator, while the coordinator waits for the callback. On close, drop
  /// only the Rust-side wait for the callback; the JavaScript promise keeps running, and the
  /// coordinator performs the complete close sequence before `watcher.close()` resolves.
  async fn await_handler_or_close<F>(&self, handler: F) -> HandlerDispatchResult
  where
    F: Future<Output = anyhow::Result<()>>,
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
    // The custom `Sleep` and consumer callbacks only impl `Future`, so fuse the
    // handler too and pin both for `select_biased!` (needs `Unpin + FusedFuture`).
    let handler = handler.fuse();
    pin_mut!(wait_for_close, handler);

    // Biased order matches tokio's `biased;`: close first, then handler.
    select_biased! {
      () = wait_for_close => HandlerDispatchResult::CloseRequested,
      result = handler => {
        if self.closed.load(Ordering::Relaxed) {
          HandlerDispatchResult::CloseRequested
        } else {
          match result {
            Ok(()) => HandlerDispatchResult::Completed,
            Err(error) => HandlerDispatchResult::Failed(error),
          }
        }
      },
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
  async fn drain_buffered_events(&mut self) -> bool {
    loop {
      // `futures`' non-deprecated `try_recv()` mirrors tokio's `try_recv()` shape
      // exactly: `Ok(msg)` = a buffered message (still drained after close while
      // any remain), `Err(TryRecvError::Empty)` = empty but open, and
      // `Err(TryRecvError::Closed)` = closed and fully drained. tokio mapped both
      // its `Empty` and `Disconnected` errors to `return true`, so the single
      // `Err(_)` arm preserves the original semantics unchanged.
      match self.rx.try_recv() {
        Ok(WatcherMsg::FileChanges { task_index, changes }) => {
          self.process_file_changes(task_index, changes).await;
        }
        Ok(WatcherMsg::Close) => {
          return false;
        }
        Err(_) => return true,
      }
    }
  }

  /// Handle close: call close_watcher hooks, close bundlers, emit close
  async fn handle_close(&mut self, mut errors: Vec<CoordinatorCloseFailure>) {
    let (new_state, should_close) = mem::take(&mut self.state).on_close();
    self.state = new_state;

    if should_close {
      // Close watcher hooks on all tasks
      for (task_index, task) in self.tasks.iter().enumerate() {
        let context = format!("watch task {task_index} closeWatcher failed");
        match AssertUnwindSafe(task.call_hook_close_watcher()).catch_unwind().await {
          Ok(Ok(())) => {}
          Ok(Err(error)) => {
            errors.extend(CoordinatorCloseFailure::from_build_error(&context, error));
          }
          Err(payload) => {
            errors.push(CoordinatorCloseFailure::from_panic(&context, payload));
          }
        }
      }

      // Close all bundlers
      for (task_index, task) in self.tasks.iter().enumerate() {
        let context = format!("watch task {task_index} closeBundle failed");
        if let Some(close_identity) = task.current_bundle_close_identity().await {
          self
            .native_owned_close_identities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(close_identity);
        }
        match AssertUnwindSafe(task.close()).catch_unwind().await {
          Ok(Ok(())) => {}
          Ok(Err(error)) => {
            errors.extend(CoordinatorCloseFailure::from_anyhow_error(&context, error));
          }
          Err(payload) => {
            errors.push(CoordinatorCloseFailure::from_panic(&context, payload));
          }
        }
      }

      match AssertUnwindSafe(self.handler.on_close()).catch_unwind().await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
          errors.extend(CoordinatorCloseFailure::from_anyhow_error(
            "watch close event handler failed",
            error,
          ));
        }
        Err(payload) => {
          errors
            .push(CoordinatorCloseFailure::from_panic("watch close event handler failed", payload));
        }
      }
    }

    if !errors.is_empty() {
      self.close_error = Some(Arc::new(CoordinatorCloseError::from_errors(errors)));
    }

    self.state = mem::take(&mut self.state).to_closed();
  }

  fn close_result(&self) -> CoordinatorCloseResult {
    self.close_error.clone().map_or(Ok(()), Err)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  // The coordinator's shared production fields changed type: `close_notify` is now
  // `Arc<event_listener::Event>` and the channel is `futures::channel::mpsc` (both
  // reach here via `use super::*`, but are imported explicitly for clarity).
  // `tokio::sync::Notify` is kept ONLY for the tests' internal end/stop signal.
  use event_listener::Event;
  use rolldown::{BundlerConfig, BundlerOptions, plugin};
  use rolldown_error::BuildResult;
  use rolldown_fs_watcher::{DynFsWatcher, FsEventHandler, FsWatcher, FsWatcherConfig, PathsMut};
  use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    sync::{
      Mutex,
      atomic::{AtomicUsize, Ordering},
    },
  };
  use tokio::sync::Notify;

  static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

  struct TestDir(PathBuf);

  impl TestDir {
    fn new() -> Self {
      let path = std::env::temp_dir().join(format!(
        "rolldown-watch-coordinator-registration-{}-{}",
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

  struct RegistrationFailingWatcher {
    fail_adds: usize,
    fail_commits: usize,
    add_attempts: Arc<AtomicUsize>,
    commit_attempts: Arc<AtomicUsize>,
    commit_times: Arc<Mutex<Vec<Instant>>>,
  }

  struct RegistrationFailingPaths {
    fail_adds: usize,
    fail_commits: usize,
    add_attempts: Arc<AtomicUsize>,
    commit_attempts: Arc<AtomicUsize>,
    commit_times: Arc<Mutex<Vec<Instant>>>,
    pending: Vec<PathBuf>,
  }

  impl PathsMut for RegistrationFailingPaths {
    fn add(
      &mut self,
      path: &Path,
      _recursive_mode: rolldown_fs_watcher::RecursiveMode,
    ) -> BuildResult<()> {
      let attempt = self.add_attempts.fetch_add(1, Ordering::SeqCst) + 1;
      if attempt <= self.fail_adds {
        return Err(anyhow::anyhow!("intentional watcher add failure {attempt}").into());
      }
      self.pending.push(path.to_path_buf());
      Ok(())
    }

    fn remove(&mut self, _path: &Path) -> BuildResult<()> {
      Ok(())
    }

    fn commit(self: Box<Self>) -> BuildResult<()> {
      self.commit_times.lock().expect("commit times lock").push(Instant::now());
      let attempt = self.commit_attempts.fetch_add(1, Ordering::SeqCst) + 1;
      if self.pending.is_empty() {
        return Ok(());
      }
      if attempt <= self.fail_commits {
        return Err(anyhow::anyhow!("intentional watcher commit failure {attempt}").into());
      }
      Ok(())
    }
  }

  impl FsWatcher for RegistrationFailingWatcher {
    fn new<F: FsEventHandler>(_event_handler: F) -> BuildResult<Self>
    where
      Self: Sized,
    {
      unreachable!("test constructs the watcher directly")
    }

    fn with_config<F: FsEventHandler>(
      _event_handler: F,
      _config: FsWatcherConfig,
    ) -> BuildResult<Self>
    where
      Self: Sized,
    {
      unreachable!("test constructs the watcher directly")
    }

    fn watch(
      &mut self,
      _path: &Path,
      _recursive_mode: rolldown_fs_watcher::RecursiveMode,
    ) -> BuildResult<()> {
      unreachable!("test uses the batch path API")
    }

    fn unwatch(&mut self, _path: &Path) -> BuildResult<()> {
      unreachable!("test never removes paths")
    }

    fn paths_mut<'me>(&'me mut self) -> Box<dyn PathsMut + 'me> {
      Box::new(RegistrationFailingPaths {
        fail_adds: self.fail_adds,
        fail_commits: self.fail_commits,
        add_attempts: Arc::clone(&self.add_attempts),
        commit_attempts: Arc::clone(&self.commit_attempts),
        commit_times: Arc::clone(&self.commit_times),
        pending: Vec::new(),
      })
    }
  }

  #[derive(Debug)]
  struct CloseProbePlugin {
    close_bundle_calls: Arc<AtomicUsize>,
  }

  impl plugin::Plugin for CloseProbePlugin {
    fn name(&self) -> Cow<'static, str> {
      "watch-registration-close-probe".into()
    }

    fn register_hook_usage(&self) -> plugin::HookUsage {
      plugin::HookUsage::CloseBundle
    }

    async fn close_bundle(
      &self,
      _ctx: &plugin::PluginContext,
      _args: Option<&plugin::HookCloseBundleArgs<'_>>,
    ) -> plugin::HookNoopReturn {
      self.close_bundle_calls.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  struct RecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
    end: Arc<Notify>,
    close_calls: Arc<AtomicUsize>,
  }

  struct RegistrationTestTask {
    task: WatchTask,
    add_attempts: Arc<AtomicUsize>,
    commit_attempts: Arc<AtomicUsize>,
    commit_times: Arc<Mutex<Vec<Instant>>>,
  }

  impl WatcherEventHandler for RecordingHandler {
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push(event.as_str().to_string());
      if matches!(event, WatchEvent::End) {
        self.end.notify_one();
      }
      Ok(())
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_restart(&self) -> anyhow::Result<()> {
      Ok(())
    }

    async fn on_close(&self) -> anyhow::Result<()> {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  fn create_task(
    test_dir: &TestDir,
    fail_adds: usize,
    fail_commits: usize,
    closed: &Arc<AtomicBool>,
    close_bundle_calls: &Arc<AtomicUsize>,
  ) -> RegistrationTestTask {
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write input");
    let input = fs::canonicalize(input).expect("canonicalize input");
    let add_attempts = Arc::new(AtomicUsize::new(0));
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let commit_times = Arc::new(Mutex::new(Vec::new()));
    let fs_watcher: DynFsWatcher = Box::new(RegistrationFailingWatcher {
      fail_adds,
      fail_commits,
      add_attempts: Arc::clone(&add_attempts),
      commit_attempts: Arc::clone(&commit_attempts),
      commit_times: Arc::clone(&commit_times),
    });
    let task = WatchTask::new(
      BundlerConfig::new(
        BundlerOptions {
          cwd: Some(test_dir.0.clone()),
          input: Some(vec![input.to_string_lossy().into_owned().into()]),
          file: Some("dist/out.js".into()),
          ..Default::default()
        },
        vec![Arc::new(CloseProbePlugin { close_bundle_calls: Arc::clone(close_bundle_calls) })],
      ),
      fs_watcher,
      closed,
    )
    .expect("create watch task");
    RegistrationTestTask { task, add_attempts, commit_attempts, commit_times }
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn coordinator_retries_scan_registration_failure_without_emitting_error() {
    let test_dir = TestDir::new();
    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());
    let close_bundle_calls = Arc::new(AtomicUsize::new(0));
    let RegistrationTestTask { task, commit_attempts, commit_times, .. } =
      create_task(&test_dir, 0, 1, &closed, &close_bundle_calls);
    let mut tasks = IndexVec::new();
    tasks.push(task);
    let events = Arc::new(Mutex::new(Vec::new()));
    let end = Arc::new(Notify::new());
    let close_calls = Arc::new(AtomicUsize::new(0));
    let coordinator = WatchCoordinator::new(
      rx,
      RecordingHandler {
        events: Arc::clone(&events),
        end: Arc::clone(&end),
        close_calls: Arc::clone(&close_calls),
      },
      tasks,
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
      Arc::default(),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(10), end.notified())
      .await
      .expect("coordinator should recover and finish the initial build");
    assert_eq!(
      commit_attempts.load(Ordering::SeqCst),
      3,
      "the retry commits both scan and render registration transactions"
    );
    {
      let commit_times = commit_times.lock().expect("commit times lock");
      assert!(
        commit_times[1].duration_since(commit_times[0]) >= WATCH_REGISTRATION_RETRY_DELAYS[0],
        "the coordinator retry must wait for its backoff"
      );
    }
    assert_eq!(
      *events.lock().expect("events lock"),
      ["START", "BUNDLE_START", "BUNDLE_END", "END"]
    );
    assert_eq!(
      close_bundle_calls.load(Ordering::SeqCst),
      1,
      "the hidden failed build must be closed before retry"
    );

    closed.store(true, Ordering::Relaxed);
    close_notify.notify(usize::MAX);
    tx.unbounded_send(WatcherMsg::Close).expect("send close");
    tokio::time::timeout(Duration::from_secs(10), handle)
      .await
      .expect("coordinator should close")
      .expect("coordinator task should not panic")
      .expect("coordinator should close successfully");
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
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

  #[tokio::test(flavor = "multi_thread")]
  async fn coordinator_retries_individual_watch_add_failure() {
    let test_dir = TestDir::new();
    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());
    let close_bundle_calls = Arc::new(AtomicUsize::new(0));
    let RegistrationTestTask { task, add_attempts, commit_attempts, .. } =
      create_task(&test_dir, 1, 0, &closed, &close_bundle_calls);
    let mut tasks = IndexVec::new();
    tasks.push(task);
    let events = Arc::new(Mutex::new(Vec::new()));
    let end = Arc::new(Notify::new());
    let close_calls = Arc::new(AtomicUsize::new(0));
    let coordinator = WatchCoordinator::new(
      rx,
      RecordingHandler {
        events: Arc::clone(&events),
        end: Arc::clone(&end),
        close_calls: Arc::clone(&close_calls),
      },
      tasks,
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
      Arc::default(),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(10), end.notified())
      .await
      .expect("coordinator should retry the failed path registration");
    assert_eq!(add_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(
      commit_attempts.load(Ordering::SeqCst),
      3,
      "the failed add attempt and both retry transactions must be committed"
    );
    assert_eq!(
      *events.lock().expect("events lock"),
      ["START", "BUNDLE_START", "BUNDLE_END", "END"]
    );
    assert_eq!(
      close_bundle_calls.load(Ordering::SeqCst),
      1,
      "the hidden failed build must be closed before retry"
    );

    closed.store(true, Ordering::Relaxed);
    close_notify.notify(usize::MAX);
    tx.unbounded_send(WatcherMsg::Close).expect("send close");
    tokio::time::timeout(Duration::from_secs(10), handle)
      .await
      .expect("coordinator should close")
      .expect("coordinator task should not panic")
      .expect("coordinator should close successfully");
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), 2);
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn coordinator_close_interrupts_registration_backoff() {
    let test_dir = TestDir::new();
    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());
    let close_bundle_calls = Arc::new(AtomicUsize::new(0));
    let RegistrationTestTask { task, commit_attempts, .. } =
      create_task(&test_dir, 0, usize::MAX, &closed, &close_bundle_calls);
    let mut tasks = IndexVec::new();
    tasks.push(task);
    let events = Arc::new(Mutex::new(Vec::new()));
    let close_calls = Arc::new(AtomicUsize::new(0));
    let coordinator = WatchCoordinator::new(
      rx,
      RecordingHandler {
        events: Arc::clone(&events),
        end: Arc::new(Notify::new()),
        close_calls: Arc::clone(&close_calls),
      },
      tasks,
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
      Arc::default(),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(10), async {
      while commit_attempts.load(Ordering::SeqCst) < 1 {
        tokio::task::yield_now().await;
      }
    })
    .await
    .expect("initial build should reach both registration attempts");
    closed.store(true, Ordering::Relaxed);
    close_notify.notify(usize::MAX);
    tx.unbounded_send(WatcherMsg::Close).expect("send close");

    tokio::time::timeout(Duration::from_secs(10), handle)
      .await
      .expect("close should interrupt registration backoff")
      .expect("coordinator task should not panic")
      .expect("explicit close should not report registration exhaustion");
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(*events.lock().expect("events lock"), ["START", "BUNDLE_START"]);
    assert_eq!(close_bundle_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn coordinator_stops_after_bounded_registration_retries() {
    let test_dir = TestDir::new();
    let (_tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());
    let close_bundle_calls = Arc::new(AtomicUsize::new(0));
    let RegistrationTestTask { task, commit_attempts, .. } =
      create_task(&test_dir, 0, usize::MAX, &closed, &close_bundle_calls);
    let mut tasks = IndexVec::new();
    tasks.push(task);
    let events = Arc::new(Mutex::new(Vec::new()));
    let close_calls = Arc::new(AtomicUsize::new(0));
    let coordinator = WatchCoordinator::new(
      rx,
      RecordingHandler {
        events: Arc::clone(&events),
        end: Arc::new(Notify::new()),
        close_calls: Arc::clone(&close_calls),
      },
      tasks,
      &WatcherConfig::default(),
      closed,
      close_notify,
      Arc::default(),
    );

    let error = tokio::time::timeout(Duration::from_secs(10), coordinator.run())
      .await
      .expect("bounded retries should terminate")
      .expect_err("exhausted watcher registration should fail closed");
    assert!(
      error.to_string().contains("watch task 0 file watcher registration failed after 3 retries")
    );
    assert!(error.to_string().contains("intentional watcher commit failure"));
    assert_eq!(error.failures().len(), 1);
    assert!(
      error.failures()[0]
        .message()
        .starts_with("watch task 0 file watcher registration failed after 3 retries:")
    );
    assert_eq!(commit_attempts.load(Ordering::SeqCst), WATCH_REGISTRATION_RETRY_DELAYS.len() + 1);
    assert_eq!(*events.lock().expect("events lock"), ["START", "BUNDLE_START"]);
    assert_eq!(
      close_bundle_calls.load(Ordering::SeqCst),
      WATCH_REGISTRATION_RETRY_DELAYS.len() + 1
    );
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn close_failures_decompose_batched_diagnostics_in_order() {
    let first = BuildDiagnostic::bundler_initialize_error("first diagnostic".to_string(), None);
    let second = BuildDiagnostic::bundler_initialize_error("second diagnostic".to_string(), None);
    let expected = [
      format!("watch close failed: {}", first.to_diagnostic()),
      format!("watch close failed: {}", second.to_diagnostic()),
    ];

    let failures = CoordinatorCloseFailure::from_build_error(
      "watch close failed",
      BatchedBuildDiagnostic::new(vec![first, second]),
    );

    assert_eq!(failures.iter().map(CoordinatorCloseFailure::message).collect::<Vec<_>>(), expected);
    assert!(failures.iter().all(|failure| {
      failure.source().is_some_and(<dyn std::error::Error + 'static>::is::<BuildDiagnostic>)
    }));

    let error = CoordinatorCloseError::from_errors(failures);
    assert_eq!(
      error.failures().iter().map(CoordinatorCloseFailure::message).collect::<Vec<_>>(),
      expected
    );
  }

  #[test]
  fn empty_diagnostic_batch_retains_contextual_failure() {
    let failures = CoordinatorCloseFailure::from_build_error(
      "watch task 0 closeBundle failed",
      BatchedBuildDiagnostic::default(),
    );

    assert_eq!(failures.len(), 1);
    assert_eq!(
      failures[0].message(),
      "watch task 0 closeBundle failed: build failed without diagnostics"
    );
    assert!(failures[0].source().is_none());
  }
}
