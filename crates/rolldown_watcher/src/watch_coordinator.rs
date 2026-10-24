use crate::event::WatchEvent;
use crate::file_change_event::FileChangeEvent;
use crate::handler::WatcherEventHandler;
use crate::watch_task::{BuildOutcome, WatchGroupIdx, WatchTask, WatchTaskIdx};
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
  group_members: IndexVec<WatchGroupIdx, Vec<WatchTaskIdx>>,
  closed: Arc<AtomicBool>,
  close_notify: Arc<Notify>,
}

impl<H: WatcherEventHandler> WatchCoordinator<H> {
  pub(crate) fn new(
    rx: mpsc::UnboundedReceiver<WatcherMsg>,
    handler: H,
    tasks: IndexVec<WatchTaskIdx, WatchTask>,
    group_members: IndexVec<WatchGroupIdx, Vec<WatchTaskIdx>>,
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
      group_members,
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
            Some(WatcherMsg::FileChanges { group_index, changes }) => {
              self.process_file_changes(group_index, changes).await;
            }
            Some(WatcherMsg::Close) => {
              self.handle_close().await;
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
                if !self.run_build_sequence(changes).await {
                  self.handle_close().await;
                  break;
                }
              }
            }
            msg = self.rx.recv() => {
              match msg {
                Some(WatcherMsg::FileChanges { group_index, changes }) => {
                  self.process_file_changes(group_index, changes).await;
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

  /// Process file changes for a config group: mark every member whose watch set contains
  /// the path (calling its on_invalidate), then batch all changes into a single state transition.
  // See internal-docs/watch-mode/implementation.md
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
  async fn drain_buffered_events(&mut self) {
    loop {
      match self.rx.try_recv() {
        Ok(WatcherMsg::FileChanges { group_index, changes }) => {
          self.process_file_changes(group_index, changes).await;
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
  use crate::task_fs_event_handler::GroupFsEventHandler;
  use rolldown::{BundlerConfig, BundlerOptions, plugin};
  use rolldown_fs_watcher::{FsWatcher, FsWatcherConfig};
  use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    sync::{
      Mutex,
      atomic::{AtomicUsize, Ordering},
    },
  };

  static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

  struct TestDir(PathBuf);

  impl TestDir {
    fn new() -> Self {
      let path = std::env::temp_dir().join(format!(
        "rolldown-watch-coordinator-group-{}-{}",
        std::process::id(),
        NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
      ));
      fs::create_dir_all(&path).expect("create test directory");
      Self(path)
    }

    /// Write `content` to `name` and return the path the resolver reports for it.
    fn write(&self, name: &str, content: &str) -> PathBuf {
      let path = self.0.join(name);
      fs::write(&path, content).expect("write input");
      dunce::canonicalize(path).expect("canonicalize input")
    }
  }

  impl Drop for TestDir {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.0);
    }
  }

  /// Records the event stream and wakes the test on the initial and the rebuild `End`.
  struct EnvelopeRecordingHandler {
    events: Arc<Mutex<Vec<String>>>,
    end_count: Arc<AtomicUsize>,
    initial_end: Arc<Notify>,
    rebuild_end: Arc<Notify>,
  }

  impl WatcherEventHandler for EnvelopeRecordingHandler {
    async fn on_event(&self, event: WatchEvent) {
      self.events.lock().expect("events lock").push(event.as_str().to_string());
      if matches!(event, WatchEvent::End) {
        if self.end_count.fetch_add(1, Ordering::SeqCst) == 0 {
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

  /// Fails every build after the first one, turning a rebuild into an `ERROR` outcome.
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

  /// Runs a coordinator whose tasks are all outputs of ONE config group (sharing one
  /// disabled fs watcher), delivers ONE `FileChanges` message for `save_path` after the
  /// initial build, and returns the recorded events and the `End` count.
  async fn run_one_group_save(
    task_inputs: Vec<(PathBuf, &str, Vec<plugin::__inner::SharedPluginable>)>,
    save_path: &Path,
  ) -> (Vec<String>, usize) {
    let (tx, rx) = mpsc::unbounded_channel();
    let closed = Arc::new(AtomicBool::new(false));
    let close_notify = Arc::new(Notify::new());
    let group_index = WatchGroupIdx::from_usize(0);

    let fs_watcher = FsWatcher::new(
      GroupFsEventHandler { group_index, tx: tx.clone() },
      &FsWatcherConfig { enabled: false, ..FsWatcherConfig::default() },
    )
    .expect("create fs watcher");
    let fs_watcher = Arc::new(Mutex::new(fs_watcher));
    let mut tasks = IndexVec::new();
    let mut members = Vec::new();
    for (input, out_file, plugins) in task_inputs {
      let options = BundlerOptions {
        cwd: Some(input.parent().expect("input has parent").to_path_buf()),
        input: Some(vec![input.to_string_lossy().into_owned().into()]),
        file: Some(out_file.into()),
        ..Default::default()
      };
      let task = WatchTask::new(
        BundlerConfig::new(options, plugins),
        tasks.next_idx(),
        Arc::clone(&fs_watcher),
        &closed,
      )
      .expect("create watch task");
      members.push(tasks.push(task));
    }
    let mut group_members = IndexVec::new();
    group_members.push(members);

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
      group_members,
      &WatcherConfig::default(),
      Arc::clone(&closed),
      Arc::clone(&close_notify),
    );
    let handle = tokio::spawn(coordinator.run());

    tokio::time::timeout(Duration::from_secs(30), initial_end.notified())
      .await
      .expect("initial build should finish");
    // ONE message for the save, as the group's shared watcher delivers it.
    tx.send(WatcherMsg::FileChanges {
      group_index,
      changes: vec![FileChangeEvent::new(
        save_path.to_string_lossy().into_owned(),
        WatcherChangeKind::Update,
      )],
    })
    .expect("send file change");
    tokio::time::timeout(Duration::from_secs(30), rebuild_end.notified())
      .await
      .expect("rebuild should finish");

    closed.store(true, Ordering::Relaxed);
    close_notify.notify_one();
    tx.send(WatcherMsg::Close).expect("send close");
    tokio::time::timeout(Duration::from_secs(30), handle)
      .await
      .expect("coordinator should close")
      .expect("coordinator task should not panic");

    let events = events.lock().expect("events lock").clone();
    (events, end_count.load(Ordering::SeqCst))
  }

  /// rolldown#10613: one save of a file watched by both outputs of one config rebuilds both
  /// inside ONE `START..END` envelope.
  #[tokio::test(flavor = "multi_thread")]
  async fn single_save_rebuilds_every_group_member_in_one_envelope() {
    let test_dir = TestDir::new();
    let input = test_dir.write("main.js", "export const value = 1;");

    let (events, end_count) = run_one_group_save(
      vec![(input.clone(), "dist0/out.js", vec![]), (input.clone(), "dist1/out.js", vec![])],
      &input,
    )
    .await;

    assert_eq!(
      events,
      [
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
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

  /// Membership stays per task: a change only one member watches rebuilds only that member.
  #[tokio::test(flavor = "multi_thread")]
  async fn group_change_hitting_one_member_rebuilds_only_that_member() {
    let test_dir = TestDir::new();
    let input_a = test_dir.write("a.js", "export const a = 1;");
    let input_b = test_dir.write("b.js", "export const b = 1;");

    let (events, end_count) = run_one_group_save(
      vec![(input_a.clone(), "dist0/out.js", vec![]), (input_b, "dist1/out.js", vec![])],
      &input_a,
    )
    .await;

    assert_eq!(
      events,
      [
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
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

  /// A member failing mid-envelope reports `ERROR`, and the envelope still ends with `End`.
  #[tokio::test(flavor = "multi_thread")]
  async fn sibling_error_mid_envelope_still_emits_end() {
    let test_dir = TestDir::new();
    let input = test_dir.write("main.js", "export const value = 1;");
    let builds = Arc::new(AtomicUsize::new(0));

    let (events, end_count) = run_one_group_save(
      vec![
        (input.clone(), "dist0/out.js", vec![]),
        (
          input.clone(),
          "dist1/out.js",
          vec![plugin::__inner::Pluginable::new_shared(FailAfterFirstBuildPlugin {
            builds: Arc::clone(&builds),
          })],
        ),
      ],
      &input,
    )
    .await;

    assert_eq!(
      events,
      [
        "START",
        "BUNDLE_START",
        "BUNDLE_END",
        "BUNDLE_START",
        "BUNDLE_END",
        "END",
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
}
