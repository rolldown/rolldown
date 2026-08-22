use crate::event::WatchEvent;
use crate::file_change_event::FileChangeEvent;
use crate::handler::WatcherEventHandler;
use crate::watch_task::{
  BuildOutcome, WatchGroupIdx, WatchTask, WatchTaskBuildError, WatchTaskIdx,
};
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
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
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
  // Biased order matters: a message queued at the deadline must win over the
  // timeout so it extends the debounce window instead of forcing an avoidable
  // intermediate build. `select_biased!` needs each branch `Unpin + FusedFuture`.
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
  /// Task membership of each config group. A `FileChanges` message carries a
  /// group identity, and every member whose watch set contains a changed path
  /// is marked, so one save covers all affected outputs of the config.
  group_members: IndexVec<WatchGroupIdx, Vec<WatchTaskIdx>>,
  /// Inverse of `group_members`: the config group each task belongs to.
  group_of: IndexVec<WatchTaskIdx, WatchGroupIdx>,
  closed: Arc<AtomicBool>,
  close_notify: Arc<Event>,
  native_owned_close_identities: Arc<Mutex<Vec<u64>>>,
  close_error: Option<Arc<CoordinatorCloseError>>,
  event_loop_errors: Vec<CoordinatorCloseFailure>,
}

impl<H: WatcherEventHandler> WatchCoordinator<H> {
  #[expect(
    clippy::too_many_arguments,
    reason = "internal constructor with one call site per production path; \
              every argument is a distinct coordinator-owned concern"
  )]
  pub(crate) fn new(
    rx: mpsc::UnboundedReceiver<WatcherMsg>,
    handler: H,
    tasks: IndexVec<WatchTaskIdx, WatchTask>,
    group_members: IndexVec<WatchGroupIdx, Vec<WatchTaskIdx>>,
    config: &WatcherConfig,
    closed: Arc<AtomicBool>,
    close_notify: Arc<Event>,
    native_owned_close_identities: Arc<Mutex<Vec<u64>>>,
  ) -> Self {
    let mut group_of: IndexVec<WatchTaskIdx, WatchGroupIdx> =
      tasks.iter().map(|_| WatchGroupIdx::from_usize(0)).collect();
    for (group_index, members) in group_members.iter_enumerated() {
      for &member in members {
        group_of[member] = group_index;
      }
    }
    Self {
      rx,
      handler,
      state: WatcherState::Idle,
      debounce_duration: config.debounce_duration(),
      tasks,
      group_members,
      group_of,
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
            Some(WatcherMsg::FileChanges { group_index, changes }) => {
              self.process_file_changes(group_index, changes).await;
            }
            Some(WatcherMsg::Close) | None => return,
          }
        }
        WatcherState::Debouncing { deadline, .. } => {
          // Must go through the timer facade: the async-runtime build has no
          // tokio reactor, so `tokio::time::sleep_until` would panic here.
          let timeout = rolldown_utils::time::sleep_until(*deadline);

          // Debounce extension rules: see internal-docs/watch-mode/implementation.md.
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
              Some(WatcherMsg::FileChanges { group_index, changes }) => {
                self.process_file_changes(group_index, changes).await;
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

    // Step 5: Build each task that needs it.
    //
    // Within one config group a save is delivered once and marks every
    // affected member, but two configs watching the same file still receive
    // independent deliveries, and a 0ms debounce deadline can fire before the
    // sibling group's message arrives. Dispatching `End` with a sibling still
    // queued would split one save into two rebuild envelopes, so before `End`
    // the queue is drained and the build pass repeats while any task still
    // needs a rebuild, keeping the save in one Start..End envelope.
    let mut dispatched_changes = changes;
    let mut rebuilt_groups: IndexVec<WatchGroupIdx, bool> =
      self.group_members.iter().map(|_| false).collect();
    loop {
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
        // The group rebuilt inside this envelope: any later message from its
        // shared watch stream is a new filesystem event, not its own report of
        // the save being handled.
        rebuilt_groups[self.group_of[task_index]] = true;
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

      // Paths reported by a group that already rebuilt in this envelope are
      // new saves, not same-save sibling reports, so they must be
      // re-dispatched below even when their (path, kind) was already reported.
      let mut new_save_paths = FxIndexSet::default();
      if !self
        .drain_buffered_events_observing(|group_index, changes| {
          if rebuilt_groups[group_index] {
            new_save_paths.extend(changes.iter().map(|change| change.path.clone()));
          }
        })
        .await
      {
        // A queued close was drained; the event loop stops and `run()`
        // performs the close sequence exactly once.
        return false;
      }
      if !self.tasks.iter().any(|task| task.needs_rebuild) {
        break;
      }

      // Consume the drained changes so they don't schedule a duplicate envelope
      // after `End`. A repeated (path, kind) from a same-save sibling stays
      // silent; a genuinely new save re-dispatches `change`/`watchChange`.
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
          for task_index in self.tasks.indices() {
            if let Err(error) = self.tasks[task_index].call_watch_change(path.as_str(), kind).await
            {
              self.event_loop_errors.extend(CoordinatorCloseFailure::from_anyhow_error(
                &format!("watch task {} watchChange failed", task_index.index()),
                error,
              ));
              return false;
            }
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
      // Listen before checking `closed`: `event_listener::Event` stores no
      // permit, so a listener created after the notify would wait forever.
      let wait_for_close = async move {
        let listener = close_notify.listen();
        if !closed.load(Ordering::Relaxed) {
          listener.await;
        }
      }
      .fuse();
      let timeout = rolldown_utils::time::sleep_until(deadline).fuse();
      pin_mut!(wait_for_close, timeout);

      // Biased: close, then message, then timeout. The message is handled after
      // the select block ends so the `&mut self.rx` borrow is released before
      // `process_file_changes` reborrows `&mut self`.
      let msg = select_biased! {
        () = wait_for_close => return false,
        msg = self.rx.next() => msg,
        () = timeout => return true,
      };
      match msg {
        Some(WatcherMsg::FileChanges { group_index, changes }) => {
          self.process_file_changes(group_index, changes).await;
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

  /// Process file changes for a config group: mark EVERY group member whose
  /// watch set contains the changed path (with its on_invalidate callback),
  /// then batch all effective changes into a single state transition. Group
  /// scoped marking is what keeps one save inside one rebuild envelope: the
  /// coordinator acts on watch-set knowledge it already owns instead of
  /// waiting for per-output notify deliveries that may straddle `End`.
  async fn process_file_changes(
    &mut self,
    group_index: WatchGroupIdx,
    changes: Vec<FileChangeEvent>,
  ) {
    let mut effective_changes: Vec<FileChangeEvent> = Vec::new();

    if let Some(members) = self.group_members.get(group_index) {
      for change in changes {
        let mut effective = false;
        for &member in members {
          let task = &mut self.tasks[member];
          if task.mark_needs_rebuild(&change.path) {
            task.call_on_invalidate(&change.path).await;
            effective = true;
          }
        }
        if effective {
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
    self.drain_buffered_events_observing(|_, _| {}).await
  }

  /// Like [`Self::drain_buffered_events`], but lets the caller observe each
  /// drained `FileChanges` message with its group provenance, which the state
  /// machine's path-keyed merge discards.
  async fn drain_buffered_events_observing(
    &mut self,
    mut observe: impl FnMut(WatchGroupIdx, &[FileChangeEvent]),
  ) -> bool {
    loop {
      // Buffered messages are still handed out after close, so drain until
      // `Err(_)` — which covers both "empty but open" and "closed and drained".
      match self.rx.try_recv() {
        Ok(WatcherMsg::FileChanges { group_index, changes }) => {
          observe(group_index, &changes);
          self.process_file_changes(group_index, changes).await;
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
  // `tokio::sync::Notify` below is ONLY the tests' internal end/stop signal;
  // production `close_notify` is `event_listener::Event`.
  use event_listener::Event;
  use rolldown::{BundlerConfig, BundlerOptions, plugin};
  use rolldown_error::BuildResult;
  use rolldown_fs_watcher::{DynFsWatcher, FsEventHandler, FsWatcher, FsWatcherConfig, PathsMut};
  use rolldown_utils::dashmap::FxDashSet;
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

  /// Group membership for tasks that each form their own config group
  /// (the multi-config layout).
  fn singleton_groups(task_count: usize) -> IndexVec<WatchGroupIdx, Vec<WatchTaskIdx>> {
    (0..task_count).map(|index| vec![WatchTaskIdx::from_usize(index)]).collect()
  }

  /// Group membership for tasks that are all outputs of ONE config group.
  fn one_group(task_count: usize) -> IndexVec<WatchGroupIdx, Vec<WatchTaskIdx>> {
    let mut groups = IndexVec::new();
    groups.push((0..task_count).map(WatchTaskIdx::from_usize).collect());
    groups
  }

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
      Arc::new(Mutex::new(fs_watcher)),
      Arc::new(FxDashSet::default()),
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
      singleton_groups(1),
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
      2,
      "the failed scan transaction and its retry commit; the post-render pass \
       has nothing new to register and must not open a transaction"
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
      group_index: WatchGroupIdx::from_usize(0),
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
      singleton_groups(1),
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
      2,
      "the failed add attempt and its retry commit; the post-render pass has \
       nothing new to register and must not open a transaction"
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
      singleton_groups(1),
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
      singleton_groups(1),
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

  struct NoopWatcher;
  struct NoopPaths;

  impl PathsMut for NoopPaths {
    fn add(
      &mut self,
      _path: &Path,
      _recursive_mode: rolldown_fs_watcher::RecursiveMode,
    ) -> BuildResult<()> {
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

    fn watch(
      &mut self,
      _path: &Path,
      _recursive_mode: rolldown_fs_watcher::RecursiveMode,
    ) -> BuildResult<()> {
      Ok(())
    }

    fn unwatch(&mut self, _path: &Path) -> BuildResult<()> {
      Ok(())
    }

    fn paths_mut<'me>(&'me mut self) -> Box<dyn PathsMut + 'me> {
      Box::new(NoopPaths)
    }
  }

  /// One filesystem save fanning out into two per-config-group `FileChanges`
  /// messages: the sibling group's message is queued during the rebuild's
  /// BundleEnd dispatch, strictly before the coordinator can dispatch `End`.
  struct SameSaveInjectingHandler {
    events: Arc<Mutex<Vec<String>>>,
    tx: mpsc::UnboundedSender<WatcherMsg>,
    inject_path: String,
    inject_group_index: WatchGroupIdx,
    injected: AtomicBool,
    end_count: Arc<AtomicUsize>,
    initial_end: Arc<Notify>,
    rebuild_end: Arc<Notify>,
  }

  impl WatcherEventHandler for SameSaveInjectingHandler {
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
      if matches!(event, WatchEvent::BundleEnd(_))
        && self.end_count.load(Ordering::SeqCst) == 1
        && !self.injected.swap(true, Ordering::SeqCst)
      {
        self
          .tx
          .unbounded_send(WatcherMsg::FileChanges {
            group_index: self.inject_group_index,
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
      Ok(())
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("CHANGE".to_string());
      Ok(())
    }

    async fn on_restart(&self) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("RESTART".to_string());
      Ok(())
    }

    async fn on_close(&self) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("CLOSE".to_string());
      Ok(())
    }
  }

  /// Counts `watchChange` plugin hook invocations.
  #[derive(Debug)]
  struct WatchChangeCountingPlugin {
    count: Arc<AtomicUsize>,
  }

  impl plugin::Plugin for WatchChangeCountingPlugin {
    fn name(&self) -> Cow<'static, str> {
      "watch-change-counter".into()
    }

    fn register_hook_usage(&self) -> plugin::HookUsage {
      plugin::HookUsage::WatchChange
    }

    async fn watch_change(
      &self,
      _ctx: &plugin::PluginContext,
      _path: &str,
      _event: WatcherChangeKind,
    ) -> plugin::HookNoopReturn {
      self.count.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  /// A genuinely new save of the same file (same kind) landing while the
  /// previous save's rebuild is still inside its envelope: queued during the
  /// rebuild's BundleEnd dispatch, strictly before `End` can be dispatched.
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
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
      if matches!(event, WatchEvent::BundleEnd(_))
        && self.end_count.load(Ordering::SeqCst) == 1
        && !self.injected.swap(true, Ordering::SeqCst)
      {
        // The first save's rebuild just finished building; the user saves the
        // file again (same path, same kind) before `End` is dispatched.
        self
          .tx
          .unbounded_send(WatcherMsg::FileChanges {
            group_index: WatchGroupIdx::from_usize(0),
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
      Ok(())
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("CHANGE".to_string());
      Ok(())
    }

    async fn on_restart(&self) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("RESTART".to_string());
      Ok(())
    }

    async fn on_close(&self) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("CLOSE".to_string());
      Ok(())
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
      Arc::new(Mutex::new(fs_watcher)),
      Arc::new(FxDashSet::default()),
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
      singleton_groups(1),
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
      Arc::default(),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(30), initial_end.notified())
      .await
      .expect("initial build should finish");

    // Save #1. Save #2 is queued by the handler while save #1's rebuild
    // BundleEnd is being dispatched.
    tx.unbounded_send(WatcherMsg::FileChanges {
      group_index: WatchGroupIdx::from_usize(0),
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
      .expect("coordinator task should not panic")
      .expect("coordinator should close successfully");

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
        // #1's build pass and triggers the second build pass.
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

  /// Two CONFIGS (two singleton groups) watching the same file: the sibling
  /// config's independent delivery lands mid-envelope and must not split the
  /// save into two rebuild envelopes. Within one config group this race is
  /// structurally impossible (one shared watch stream marks every member), so
  /// this pins the remaining cross-config drain behavior.
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
        Arc::new(Mutex::new(fs_watcher)),
        Arc::new(FxDashSet::default()),
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
        inject_group_index: WatchGroupIdx::from_usize(1),
        injected: AtomicBool::new(false),
        end_count: Arc::clone(&end_count),
        initial_end: Arc::clone(&initial_end),
        rebuild_end: Arc::clone(&rebuild_end),
      },
      tasks,
      singleton_groups(2),
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
      Arc::default(),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(30), initial_end.notified())
      .await
      .expect("initial build should finish");

    // The first per-config message of the save. Its sibling for config group 1
    // is queued by the handler while group 0's rebuild BundleEnd is being
    // dispatched.
    tx.unbounded_send(WatcherMsg::FileChanges {
      group_index: WatchGroupIdx::from_usize(0),
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
      .expect("coordinator task should not panic")
      .expect("coordinator should close successfully");

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

  /// Records the full event stream and distinguishes the initial build's `End`
  /// from the rebuild's `End`. Injects nothing.
  struct EnvelopeRecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
    end_count: Arc<AtomicUsize>,
    initial_end: Arc<Notify>,
    rebuild_end: Arc<Notify>,
  }

  impl WatcherEventHandler for EnvelopeRecordingHandler {
    async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push(event.as_str().to_string());
      if matches!(event, WatchEvent::End) {
        let ends = self.end_count.fetch_add(1, Ordering::SeqCst) + 1;
        if ends == 1 {
          self.initial_end.notify_one();
        } else {
          self.rebuild_end.notify_one();
        }
      }
      Ok(())
    }

    async fn on_change(&self, _path: &str, _kind: WatcherChangeKind) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("CHANGE".to_string());
      Ok(())
    }

    async fn on_restart(&self) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("RESTART".to_string());
      Ok(())
    }

    async fn on_close(&self) -> anyhow::Result<()> {
      self.events.lock().expect("events lock").push("CLOSE".to_string());
      Ok(())
    }
  }

  /// Fails every build after the first one succeeded, turning a sibling's
  /// rebuild into an `ERROR` outcome.
  #[derive(Debug)]
  struct FailAfterFirstBuildPlugin {
    builds: Arc<AtomicUsize>,
  }

  impl plugin::Plugin for FailAfterFirstBuildPlugin {
    fn name(&self) -> Cow<'static, str> {
      "fail-after-first-build".into()
    }

    fn register_hook_usage(&self) -> plugin::HookUsage {
      plugin::HookUsage::BuildStart
    }

    async fn build_start(
      &self,
      _ctx: &plugin::PluginContext,
      _args: &plugin::HookBuildStartArgs<'_>,
    ) -> plugin::HookNoopReturn {
      if self.builds.fetch_add(1, Ordering::SeqCst) >= 1 {
        anyhow::bail!("intentional sibling rebuild failure");
      }
      Ok(())
    }
  }

  struct GroupEnvelopeFixture {
    tx: mpsc::UnboundedSender<WatcherMsg>,
    closed: Arc<AtomicBool>,
    close_notify: Arc<Event>,
    events: Arc<Mutex<Vec<String>>>,
    end_count: Arc<AtomicUsize>,
    initial_end: Arc<Notify>,
    rebuild_end: Arc<Notify>,
    handle: tokio::task::JoinHandle<CoordinatorCloseResult>,
  }

  /// Spawn a coordinator whose tasks all belong to ONE config group. Each
  /// entry of `task_inputs` is `(input_path, out_file, plugins)`.
  fn spawn_one_group_coordinator(
    task_inputs: Vec<(PathBuf, &str, Vec<plugin::__inner::SharedPluginable>)>,
  ) -> GroupEnvelopeFixture {
    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());

    let mut tasks = IndexVec::new();
    for (input, out_file, plugins) in task_inputs {
      let cwd = input.parent().expect("input has parent").to_path_buf();
      let fs_watcher: DynFsWatcher = Box::new(NoopWatcher);
      let task = WatchTask::new(
        BundlerConfig::new(
          BundlerOptions {
            cwd: Some(cwd),
            input: Some(vec![input.to_string_lossy().into_owned().into()]),
            file: Some(out_file.into()),
            ..Default::default()
          },
          plugins,
        ),
        Arc::new(Mutex::new(fs_watcher)),
        Arc::new(FxDashSet::default()),
        &closed,
      )
      .expect("create watch task");
      tasks.push(task);
    }
    let task_count = tasks.len();

    let events = Arc::new(Mutex::new(Vec::new()));
    let end_count = Arc::new(AtomicUsize::new(0));
    let initial_end = Arc::new(Notify::new());
    let rebuild_end = Arc::new(Notify::new());
    let coordinator = WatchCoordinator::new(
      rx,
      EnvelopeRecordingHandler {
        events: Arc::clone(&events),
        end_count: Arc::clone(&end_count),
        initial_end: Arc::clone(&initial_end),
        rebuild_end: Arc::clone(&rebuild_end),
      },
      tasks,
      one_group(task_count),
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
      Arc::default(),
    );
    let handle = tokio::spawn(coordinator.run());
    GroupEnvelopeFixture {
      tx,
      closed,
      close_notify,
      events,
      end_count,
      initial_end,
      rebuild_end,
      handle,
    }
  }

  async fn run_one_group_save(
    fixture: GroupEnvelopeFixture,
    save_path: String,
  ) -> (Vec<String>, usize) {
    tokio::time::timeout(Duration::from_secs(30), fixture.initial_end.notified())
      .await
      .expect("initial build should finish");

    // ONE message for the save: the config group's shared watch stream
    // delivers a save exactly once, carrying the group identity.
    fixture
      .tx
      .unbounded_send(WatcherMsg::FileChanges {
        group_index: WatchGroupIdx::from_usize(0),
        changes: vec![FileChangeEvent::new(save_path, WatcherChangeKind::Update)],
      })
      .expect("send file change");

    tokio::time::timeout(Duration::from_secs(30), fixture.rebuild_end.notified())
      .await
      .expect("rebuild should finish");

    fixture.closed.store(true, Ordering::Relaxed);
    fixture.close_notify.notify(usize::MAX);
    fixture.tx.unbounded_send(WatcherMsg::Close).expect("send close");
    tokio::time::timeout(Duration::from_secs(30), fixture.handle)
      .await
      .expect("coordinator should close")
      .expect("coordinator task should not panic")
      .expect("coordinator should close successfully");

    let events = fixture.events.lock().expect("events lock").clone();
    (events, fixture.end_count.load(Ordering::SeqCst))
  }

  /// Deterministic pin for rolldown#10613: ONE save of a file watched by both
  /// output tasks of one config produces ONE rebuild envelope covering both
  /// outputs. The pre-fix per-task watcher fan-out cannot guarantee this: the
  /// envelope could complete between the two independent deliveries, emitting
  /// `End` with the sibling stale and a second envelope after it.
  #[tokio::test(flavor = "multi_thread")]
  async fn single_save_rebuilds_every_group_member_in_one_envelope() {
    let test_dir = TestDir::new();
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write input");
    let input = fs::canonicalize(input).expect("canonicalize input");
    let input_str = input.to_string_lossy().into_owned();

    let fixture = spawn_one_group_coordinator(vec![
      (input.clone(), "dist0/out.js", vec![]),
      (input.clone(), "dist1/out.js", vec![]),
    ]);
    let (events, end_count) = run_one_group_save(fixture, input_str).await;

    assert_eq!(
      events,
      [
        // Initial build: both outputs in one envelope.
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        // The save's single group-scoped message marks both members, so the
        // rebuild envelope covers both outputs before `End`.
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
    assert_eq!(end_count, 2);
  }

  /// A group-scoped delivery still respects each member's own watch set: a
  /// change hitting only one member rebuilds only that member, and `End`
  /// still fires for the envelope.
  #[tokio::test(flavor = "multi_thread")]
  async fn group_change_hitting_one_member_rebuilds_only_that_member() {
    let test_dir = TestDir::new();
    let input_a = test_dir.0.join("a.js");
    let input_b = test_dir.0.join("b.js");
    fs::write(&input_a, "export const a = 1;").expect("write input a");
    fs::write(&input_b, "export const b = 1;").expect("write input b");
    let input_a = fs::canonicalize(input_a).expect("canonicalize input a");
    let input_b = fs::canonicalize(input_b).expect("canonicalize input b");
    let input_a_str = input_a.to_string_lossy().into_owned();

    let fixture = spawn_one_group_coordinator(vec![
      (input_a.clone(), "dist0/out.js", vec![]),
      (input_b.clone(), "dist1/out.js", vec![]),
    ]);
    let (events, end_count) = run_one_group_save(fixture, input_a_str).await;

    assert_eq!(
      events,
      [
        // Initial build: both members.
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        // Only member 0 watches a.js: one BundleStart/BundleEnd pair, then
        // `End` without waiting on the unaffected member.
        "CHANGE",
        "RESTART",
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        "CLOSE",
      ]
    );
    assert_eq!(end_count, 2);
  }

  /// A sibling member failing mid-envelope reports `ERROR` in place of its
  /// `BundleEnd`, and the envelope still terminates with `End` — no hang.
  #[tokio::test(flavor = "multi_thread")]
  async fn sibling_error_mid_envelope_still_emits_end() {
    let test_dir = TestDir::new();
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write input");
    let input = fs::canonicalize(input).expect("canonicalize input");
    let input_str = input.to_string_lossy().into_owned();

    let builds = Arc::new(AtomicUsize::new(0));
    let fixture = spawn_one_group_coordinator(vec![
      (input.clone(), "dist0/out.js", vec![]),
      (
        input.clone(),
        "dist1/out.js",
        vec![Arc::new(FailAfterFirstBuildPlugin { builds: Arc::clone(&builds) })],
      ),
    ]);
    let (events, end_count) = run_one_group_save(fixture, input_str).await;

    assert_eq!(
      events,
      [
        // Initial build succeeds for both members.
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        // Member 1's rebuild fails: `Error` replaces its `BundleEnd`, and the
        // envelope still closes with `End`.
        "CHANGE",
        "RESTART",
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "ERROR",
        "END",
        "CLOSE",
      ]
    );
    assert_eq!(end_count, 2);
    assert_eq!(builds.load(Ordering::SeqCst), 2, "member 1 must attempt exactly two builds");
  }

  /// Two output tasks of ONE config group share one backend watcher and build
  /// sequentially over ~the same module graph, so the sibling rediscovers
  /// every path the first member already registered. The sibling must adopt
  /// the group's registration instead of opening another paths transaction:
  /// on macOS a transaction restarts the shared FSEvents stream and drops the
  /// events buffered while it is open, blinding the whole group.
  #[tokio::test(flavor = "multi_thread")]
  async fn group_sibling_adopts_shared_registration_with_a_single_commit() {
    let test_dir = TestDir::new();
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write input");
    let input = fs::canonicalize(input).expect("canonicalize input");
    let input_str = input.to_string_lossy().into_owned();

    let (tx, rx) = mpsc::unbounded();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Event::new());
    let add_attempts = Arc::new(AtomicUsize::new(0));
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let fs_watcher: DynFsWatcher = Box::new(RegistrationFailingWatcher {
      fail_adds: 0,
      fail_commits: 0,
      add_attempts: Arc::clone(&add_attempts),
      commit_attempts: Arc::clone(&commit_attempts),
      commit_times: Arc::new(Mutex::new(Vec::new())),
    });
    let fs_watcher = Arc::new(Mutex::new(fs_watcher));
    let group_registered_files = Arc::new(FxDashSet::default());

    let mut tasks = IndexVec::new();
    for out_file in ["dist0/out.js", "dist1/out.js"] {
      let task = WatchTask::new(
        BundlerConfig::new(
          BundlerOptions {
            cwd: Some(test_dir.0.clone()),
            input: Some(vec![input_str.clone().into()]),
            file: Some(out_file.into()),
            ..Default::default()
          },
          vec![],
        ),
        Arc::clone(&fs_watcher),
        Arc::clone(&group_registered_files),
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
      EnvelopeRecordingHandler {
        events: Arc::clone(&events),
        end_count: Arc::clone(&end_count),
        initial_end: Arc::clone(&initial_end),
        rebuild_end: Arc::clone(&rebuild_end),
      },
      tasks,
      one_group(2),
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
      Arc::default(),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(30), initial_end.notified())
      .await
      .expect("initial build should finish");
    assert_eq!(
      commit_attempts.load(Ordering::SeqCst),
      1,
      "the group's whole initial build must open exactly one backend transaction: \
       the sibling adopts the already-registered paths instead of re-registering them"
    );
    assert_eq!(add_attempts.load(Ordering::SeqCst), 1, "the shared path is added exactly once");

    // Adoption must preserve marking: the shared path lives in the sibling's
    // own watch set, so one group-scoped delivery rebuilds both members.
    tx.unbounded_send(WatcherMsg::FileChanges {
      group_index: WatchGroupIdx::from_usize(0),
      changes: vec![FileChangeEvent::new(input_str.clone(), WatcherChangeKind::Update)],
    })
    .expect("send file change");
    tokio::time::timeout(Duration::from_secs(30), rebuild_end.notified())
      .await
      .expect("rebuild should finish");

    closed.store(true, Ordering::Relaxed);
    close_notify.notify(usize::MAX);
    tx.unbounded_send(WatcherMsg::Close).expect("send close");
    tokio::time::timeout(Duration::from_secs(30), handle)
      .await
      .expect("coordinator should close")
      .expect("coordinator task should not panic")
      .expect("coordinator should close successfully");

    assert_eq!(
      *events.lock().expect("events lock"),
      [
        // Initial build: both members, one envelope, one backend commit.
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
        // Both members rebuild on the shared path's change.
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
    assert_eq!(
      commit_attempts.load(Ordering::SeqCst),
      1,
      "steady-state rebuilds re-register nothing and must not reopen the transaction"
    );
  }
}
