use std::{
  collections::VecDeque,
  path::{Path, PathBuf},
  sync::{Arc, Mutex as StdMutex, atomic::AtomicU32},
};

use anyhow::Context;
use arcstr::ArcStr;
use async_lock::Mutex;
use futures::StreamExt;
use rolldown_common::{WatchPath, WatcherChangeKind};
use rolldown_dev_common::types::{DevCallbackError, DevCallbackResult};
use rolldown_error::BuildResult;
use rolldown_fs_watcher::{FsEvent, FsWatcher};
use rolldown_utils::{
  futures::spawn_detached,
  indexmap::{FxIndexMap, FxIndexSet},
  pattern_filter,
};

use rolldown::Bundler;

use crate::{
  bundling_task::BundlingTask,
  dev_context::{
    BundlingFuture, SharedDevContext, dev_callback_result_to_build_result, merge_build_results,
  },
  type_aliases::{CoordinatorReceiver, CoordinatorSender},
  types::{
    coordinator_msg::CoordinatorMsg, coordinator_state::CoordinatorState,
    coordinator_state_snapshot::CoordinatorStateSnapshot,
    ensure_latest_bundle_output_return::EnsureLatestBundleOutputReturn, error_stage::ErrorStage,
    schedule_build_return::ScheduleBuildReturn, task_input::TaskInput,
  },
  watcher_event_handler::WatcherEventHandler,
};

/// BundleCoordinator - coordinates build tasks and manages initial build state
pub struct BundleCoordinator {
  bundler: Arc<Mutex<Bundler>>,
  ctx: SharedDevContext,
  /// The engine-wide patch-id counter (shared with lazy compiles) — see the
  /// field doc on `DevEngine::next_hmr_patch_id`.
  next_hmr_patch_id: Arc<AtomicU32>,
  rx: CoordinatorReceiver,
  watcher: StdMutex<FsWatcher>,
  /// Tracks the state of the initial build
  state: CoordinatorState,
  /// File changes that arrived during initial build
  queued_file_changes_waited_for_full_build: FxIndexMap<PathBuf, WatcherChangeKind>,
  /// Build state - managed directly by coordinator
  queued_tasks: VecDeque<TaskInput>,
  has_stale_bundle_output: bool,
  current_bundling_future: Option<BundlingFuture>,
  last_callback_error: Option<DevCallbackError>,
}

impl BundleCoordinator {
  pub fn new(
    bundler: Arc<Mutex<Bundler>>,
    ctx: SharedDevContext,
    rx: CoordinatorReceiver,
    watcher: FsWatcher,
    next_hmr_patch_id: Arc<AtomicU32>,
  ) -> Self {
    Self {
      bundler,
      ctx,
      next_hmr_patch_id,
      rx,
      watcher: StdMutex::new(watcher),
      state: CoordinatorState::Initialized,
      queued_file_changes_waited_for_full_build: FxIndexMap::default(),
      // Initialize build state with initial build task
      queued_tasks: VecDeque::from([]),
      has_stale_bundle_output: true,
      current_bundling_future: None,
      last_callback_error: None,
    }
  }

  /// Create a watcher event handler that sends file change events to this coordinator
  pub fn create_watcher_event_handler(coordinator_tx: CoordinatorSender) -> WatcherEventHandler {
    WatcherEventHandler { coordinator_tx }
  }

  /// Run the coordinator message loop
  pub async fn run(mut self) {
    match self.state {
      CoordinatorState::Initialized => {
        // Start with initial build
        self.queued_tasks.push_back(TaskInput::FullBuild);
        // FIXME: hyf0: doesn't feel right to set state here before scheduling build
        self.set_initial_build_state(CoordinatorState::Idle);
        self.schedule_build_if_stale().await;
      }
      _ => {
        tracing::error!(
          "[BundleCoordinator] started in unexpected state and was terminated\n - state: {:?}",
          self.state
        );
        return;
      }
    }
    tracing::trace!("[BundleCoordinator] starts running\n - state: {:?}", self.state);
    while let Some(msg) = self.rx.next().await {
      tracing::trace!("[BundleCoordinator] received message\n - message: {msg:#?}");
      match msg {
        CoordinatorMsg::WatchEvent(watch_event) => {
          self.handle_watch_event(watch_event).await;
        }
        CoordinatorMsg::BundleCompleted {
          error_stage,
          has_generated_bundle_output,
          callback_error,
          watch_files,
        } => {
          self
            .handle_bundle_completed(
              error_stage,
              has_generated_bundle_output,
              callback_error,
              &watch_files,
            )
            .await;
        }
        #[cfg(feature = "testing")]
        CoordinatorMsg::ScheduleBuildIfStale { reply } => {
          let result = self.schedule_build_if_stale().await;
          let _ = reply.send(result);
        }
        CoordinatorMsg::GetState { reply } => {
          let status = self.create_state_snapshot();
          let _ = reply.send(status);
        }
        CoordinatorMsg::EnsureLatestBundleOutput { reply } => {
          let result = self.ensure_latest_bundle_output().await;
          let _ = reply.send(result);
        }
        CoordinatorMsg::TriggerFullBuild => {
          self.trigger_full_build().await;
        }
        #[cfg(feature = "testing")]
        CoordinatorMsg::GetWatchedFiles { reply } => {
          let result = self
            .watcher
            .lock()
            .map(|watcher| {
              watcher.watched_paths().map(|path| path.to_string_lossy().into_owned()).collect()
            })
            .unwrap_or_default();
          let _ = reply.send(result);
        }
        CoordinatorMsg::ModuleChanged { module_id, watch_files } => {
          self.handle_module_changed(module_id, &watch_files).await;
        }
        CoordinatorMsg::Close { reply } => {
          let result = self.close().await;
          let _ = reply.send(result);
          break;
        }
      }
    }
  }

  /// Handle programmatic module change (e.g., lazy compilation executed).
  ///
  /// `watch_files` is the sender's snapshot of `plugin_driver.watch_files`,
  /// unioned with whatever the live handle holds - see
  /// `CoordinatorMsg::ModuleChanged` and
  /// `internal-docs/dev-engine/implementation.md`.
  async fn handle_module_changed(&mut self, module_id: String, watch_files: &[ArcStr]) {
    let _ = self.update_watch_paths_including(watch_files).await;

    let mut changed_files = FxIndexMap::default();
    changed_files.insert(PathBuf::from(&module_id), WatcherChangeKind::Update);

    self.queued_tasks.push_back(TaskInput::Rebuild { changed_files });
    self.has_stale_bundle_output = true;

    let _ = self.schedule_build_if_stale().await;
  }

  async fn close(&mut self) -> BuildResult<()> {
    // A running task may replace `last_bundle_handle` after its HMR
    // stage. Wait for the complete task before closing the bundler so
    // `closeBundle` always runs on the final installed plugin driver.
    // See internal-docs/dev-engine/implementation.md.
    let callback_result = if let Some(bundling_future) = self.current_bundling_future.take() {
      bundling_future.await
    } else if let Some(error) = self.last_callback_error.take() {
      Err(error)
    } else {
      Ok(())
    };
    let close_result = {
      let mut bundler = self.bundler.lock().await;
      bundler.close().await
    };
    Self::merge_callback_and_close_results(callback_result, close_result)
  }

  /// Handle file change events from watcher.
  ///
  /// See `internal-docs/dev-engine/implementation.md` ("From fs event to queued task").
  async fn handle_watch_event(&mut self, events: Vec<FsEvent>) {
    let changed_files = events.into_iter().map(|event| (event.path, event.kind)).collect();
    self.handle_file_changes(changed_files).await;
  }

  /// Handle file changes based on initial build state
  async fn handle_file_changes(&mut self, changed_files: FxIndexMap<PathBuf, WatcherChangeKind>) {
    if changed_files.is_empty() {
      return;
    }

    match self.state {
      // If initial build in progress, queue the file changes
      CoordinatorState::FullBuildInProgress => {
        self.queued_file_changes_waited_for_full_build.extend(changed_files);
      }
      CoordinatorState::Idle | CoordinatorState::InProgress => {
        let task_input = if self.ctx.options.rebuild_strategy.is_always() {
          TaskInput::HmrRebuild { changed_files }
        } else {
          TaskInput::Hmr { changed_files }
        };

        self.queued_tasks.push_back(task_input);

        let _ = self.schedule_build_if_stale().await;
      }
      CoordinatorState::Failed { last_error_stage } => {
        // Mental model: if the file is edited twice and the first edit was invalid,
        // we treat the second edit as the only edit and follow the usual flow.
        //
        // Recovery choice (per Design principles §3 corollary): a Rebuild-stage
        // failure left the bundle output stale w.r.t. source, so the recovery
        // task must include a rebuild. An Hmr-stage failure (incl. watch_change
        // hook) is recoverable by re-running the Hmr task alone.
        let force_rebuild = matches!(last_error_stage, ErrorStage::Rebuild);
        let task_input = if force_rebuild || self.ctx.options.rebuild_strategy.is_always() {
          TaskInput::HmrRebuild { changed_files }
        } else {
          TaskInput::Hmr { changed_files }
        };

        self.queued_tasks.push_back(task_input);

        let _ = self.schedule_build_if_stale().await;
      }
      CoordinatorState::FullBuildFailed => {
        tracing::warn!(
          "[BundleCoordinator] received file changes while in FullBuildFailed state - scheduling full build"
        );
        // Clear the queued file changes - they'll be picked up by the full build
        self.queued_file_changes_waited_for_full_build.clear();
        self.queued_tasks.push_back(TaskInput::FullBuild);
        let _ = self.schedule_build_if_stale().await;
      }
      CoordinatorState::Initialized => {
        // Should not receive file changes in Initialized state
        tracing::error!(
          "[BundleCoordinator] received file changes in Initialized state - ignoring"
        );
      }
    }
  }

  /// Handle build completion notification
  ///
  /// `watch_files` is the task's snapshot of the handle its rebuild retired,
  /// unioned with the live handle for the same reason `ModuleChanged` carries
  /// one — see `CoordinatorMsg::BundleCompleted`.
  async fn handle_bundle_completed(
    &mut self,
    error_stage: Option<ErrorStage>,
    has_generated_bundle_output: bool,
    callback_error: Option<DevCallbackError>,
    watch_files: &[ArcStr],
  ) {
    self.last_callback_error = callback_error;
    match self.state {
      CoordinatorState::Initialized
      | CoordinatorState::Failed { .. }
      | CoordinatorState::FullBuildFailed
      | CoordinatorState::Idle => {
        tracing::error!(
          "[BundleCoordinator] received bundle completed in unexpected state and was ignored\n - state: {:?}",
          self.state
        );
      }
      CoordinatorState::FullBuildInProgress => {
        self.current_bundling_future = None;

        // Even if the build failed, update the watch paths
        // so that a new full build is triggered by the change for those files
        let _ = self.update_watch_paths_including(watch_files).await;

        if error_stage.is_some() {
          // FullBuildFailed always recovers via FullBuild on next file change,
          // so the originating stage is not tracked.
          self.set_initial_build_state(CoordinatorState::FullBuildFailed);
          self.has_stale_bundle_output = true;
        } else {
          self.has_stale_bundle_output = false;

          self.set_initial_build_state(CoordinatorState::Idle);
          if !self.queued_file_changes_waited_for_full_build.is_empty() {
            let queued_changes =
              std::mem::take(&mut self.queued_file_changes_waited_for_full_build);
            self.handle_file_changes(queued_changes).await;
          }
        }
        // We wouldn't try to schedule next build for FullBuildInProgress
        // - If it failed, we wait for external trigger
        // - If it succeeded, we already handled queued file changes above
      }
      CoordinatorState::InProgress => {
        // Clear current build
        self.current_bundling_future = None;

        // Register any new files this rebuild pulled into `watch_files`
        // (e.g. an edit that introduced a new transitive import).
        let _ = self.update_watch_paths_including(watch_files).await;

        if let Some(stage) = error_stage {
          self.set_initial_build_state(CoordinatorState::Failed { last_error_stage: stage });
          self.has_stale_bundle_output = true;
        } else {
          self.has_stale_bundle_output = !has_generated_bundle_output;

          self.set_initial_build_state(CoordinatorState::Idle);
        }
        // Succeed or fail, always try to schedule next build as it might fix the error
        let _ = self.schedule_build_if_stale().await;
      }
    }
  }

  /// Schedule a build to consume pending changed files
  #[expect(clippy::unused_async)]
  async fn schedule_build_if_stale(&mut self) -> Option<ScheduleBuildReturn> {
    tracing::trace!("[BundleCoordinator] scheduling build if stale\n - state: {:?}", self.state);
    match self.state {
      CoordinatorState::Initialized => {
        tracing::error!(
          "[BundleCoordinator] cannot schedule build when in Initialized state - coordinator misused\n - state: {:?}",
          self.state
        );
        None
      }

      CoordinatorState::FullBuildInProgress | CoordinatorState::InProgress => {
        tracing::trace!(
          "[BundleCoordinator] found running build - skipping scheduling\n - state: {:?}",
          self.state
        );
        // If there's build running, it will be responsible to handle new changed files.
        // So, we only need to wait for the latest build to finish.
        Some(ScheduleBuildReturn { future: self.current_bundling_future.clone().unwrap() })
      }
      CoordinatorState::Idle
      | CoordinatorState::FullBuildFailed
      | CoordinatorState::Failed { .. } => {
        if let Some(mut task_input) = self.queued_tasks.pop_front() {
          tracing::trace!(
            "[BundleCoordinator] scheduling new build task\n - state: {:?}\n - task_input: {task_input:#?}",
            self.state
          );
          let mut merged_task_count = 0;
          // Merge mergeable task inputs into one.
          while let Some(peeked) = self.queued_tasks.pop_front() {
            if task_input.is_mergeable_with(&peeked) {
              task_input.merge_with(peeked);
              merged_task_count += 1;
            } else {
              self.queued_tasks.push_front(peeked);
              break;
            }
          }
          if merged_task_count > 0 {
            tracing::trace!(
              "[BundleCoordinator] merged {merged_task_count} extra tasks into one\n - merged_task_input: {task_input:#?}"
            );
          }

          let bundling_task = BundlingTask::new(
            task_input,
            Arc::clone(&self.bundler),
            Arc::clone(&self.ctx),
            Arc::clone(&self.next_hmr_patch_id),
          );
          if bundling_task.input.requires_full_rebuild() {
            self.set_initial_build_state(CoordinatorState::FullBuildInProgress);
          } else {
            self.set_initial_build_state(CoordinatorState::InProgress);
          }
          self.last_callback_error = None;
          let bundling_future = BundlingFuture::new(bundling_task.run());
          let detached_bundling_future = bundling_future.clone();
          spawn_detached(detached_bundling_future.drive());

          self.current_bundling_future = Some(bundling_future.clone());

          Some(ScheduleBuildReturn { future: bundling_future })
        } else {
          tracing::trace!(
            "[BundleCoordinator] doesn't have any build to schedule\n - state: {:?}",
            self.state
          );
          None
        }
      }
    }
  }

  /// Ensure latest bundle output is available
  /// Returns Some(EnsureLatestBundleOutputReturn) if there's a build to wait for, None if output is already fresh
  async fn ensure_latest_bundle_output(&mut self) -> Option<EnsureLatestBundleOutputReturn> {
    tracing::trace!("[BundleCoordinator] is ensuring latest bundle output");
    match self.state {
      CoordinatorState::Initialized => {
        tracing::warn!(
          "[BundleCoordinator] cannot ensure latest bundle output when in Initialized state - coordinator misused\n - state: {:?}",
          self.state
        );
        None
      }
      CoordinatorState::Idle => {
        if self.queued_tasks.is_empty() {
          if self.has_stale_bundle_output {
            tracing::trace!(
              "[BundleCoordinator] output is stale, scheduling build to ensure latest output"
            );
            self
              .queued_tasks
              .push_back(TaskInput::Rebuild { changed_files: FxIndexMap::default() });
            let schedule_result = self.schedule_build_if_stale().await;
            schedule_result.map(|ret| EnsureLatestBundleOutputReturn {
              future: ret.future,
              is_ensure_latest_bundle_output_future: true,
            })
          } else {
            tracing::trace!(
              "[BundleCoordinator] output is fresh, no build needed to ensure latest output"
            );
            None
          }
        } else {
          let schedule_result = self.schedule_build_if_stale().await;
          schedule_result.map(|ret| EnsureLatestBundleOutputReturn {
            future: ret.future,
            is_ensure_latest_bundle_output_future: false,
          })
        }
      }
      CoordinatorState::FullBuildInProgress | CoordinatorState::InProgress => {
        tracing::trace!("[BundleCoordinator] found running build and end ensuring");
        // If there's a build running, return its future
        Some(EnsureLatestBundleOutputReturn {
          future: self.current_bundling_future.clone().unwrap(),
          is_ensure_latest_bundle_output_future: false,
        })
      }
      CoordinatorState::FullBuildFailed | CoordinatorState::Failed { .. } => {
        // Don't auto-retry — without file changes the same error would recur.
        // Recovery is driven by file change events from the watcher (see handle_file_changes).
        None
      }
    }
  }

  /// Unconditionally schedule a full build, regardless of current state.
  /// Used for explicit manual retry (e.g., dev server `r` signal).
  async fn trigger_full_build(&mut self) {
    self.queued_tasks.clear();
    self.queued_tasks.push_back(TaskInput::FullBuild);
    self.schedule_build_if_stale().await;
  }

  /// Get current build status - atomic operation that doesn't block
  fn create_state_snapshot(&self) -> CoordinatorStateSnapshot {
    let last_build_errored =
      matches!(self.state, CoordinatorState::Failed { .. } | CoordinatorState::FullBuildFailed);
    let last_error_stage = match self.state {
      CoordinatorState::Failed { last_error_stage } => Some(last_error_stage),
      _ => None,
    };
    CoordinatorStateSnapshot {
      running_future: self.current_bundling_future.clone(),
      last_build_errored,
      last_error_stage,
      last_callback_error: self.last_callback_error.clone(),
      has_stale_output: self.has_stale_bundle_output,
    }
  }

  /// The callback failure comes first, then the `closeBundle` failure.
  fn merge_callback_and_close_results(
    callback_result: DevCallbackResult,
    close_result: BuildResult<()>,
  ) -> BuildResult<()> {
    merge_build_results(dev_callback_result_to_build_result(callback_result), close_result)
  }

  fn set_initial_build_state(&mut self, new_state: CoordinatorState) {
    self.state = new_state;
  }

  /// Register `extra` **in addition to** the current bundle handle's watch
  /// files.
  ///
  /// The two sets are unioned, never substituted. Each covers what the other
  /// can miss: `extra` is a snapshot one producer took under the bundler lock,
  /// so it says nothing about the paths every other producer has contributed
  /// since; the live handle is authoritative about those but may already have
  /// been replaced by a rebuild, which installs a fresh, empty
  /// `plugin_driver.watch_files` and drops whatever `extra` was capturing.
  /// Taking either one alone loses registrations, in opposite directions.
  ///
  /// Registration is monotone: the watcher's path set only grows and nothing
  /// is ever unwatched, so re-offering a path that is already registered is
  /// free and a path that arrives from both sources is added once.
  async fn update_watch_paths_including(&self, extra: &[ArcStr]) -> BuildResult<()> {
    let (mut watch_files, cwd) = {
      let bundler = self.bundler.lock().await;
      (
        bundler
          .watch_files()
          .iter()
          .map(|watch_file| watch_file.clone())
          .collect::<FxIndexSet<_>>(),
        bundler.options().cwd.clone(),
      )
    };
    watch_files.extend(extra.iter().cloned());
    let watch_files = watch_files.into_iter().collect::<Vec<_>>();

    let include = self.ctx.options.watch_include.as_deref();
    let exclude = self.ctx.options.watch_exclude.as_deref();

    Self::update_watch_paths_from(&self.watcher, &watch_files, &cwd, include, exclude)
  }

  fn update_watch_paths_from(
    watcher: &StdMutex<FsWatcher>,
    watch_files: &[ArcStr],
    cwd: &Path,
    include: Option<&[rolldown_utils::pattern_filter::StringOrRegex]>,
    exclude: Option<&[rolldown_utils::pattern_filter::StringOrRegex]>,
  ) -> BuildResult<()> {
    let cwd_str = cwd.to_string_lossy();
    let mut watcher = watcher.lock().ok().context("Failed to acquire watcher lock")?;
    // `addWatchFile` accepts nonexistent and virtual paths, so a refused
    // registration is skipped rather than failing the build; it never enters
    // the watcher's path set, so later builds offer it again. The batch is
    // always committed. A commit failure is returned and the coordinator
    // ignores it (`let _ =`); none of the batch is recorded, so the next build
    // offers those paths again. See internal-docs/dev-engine/implementation.md.
    let watch_paths: Vec<WatchPath> =
      watch_files.iter().map(|watch_file| WatchPath::new(watch_file.as_str(), cwd)).collect();
    watcher.watch_paths(watch_paths.iter().map(WatchPath::as_path), |path| {
      pattern_filter::filter(exclude, include, &path.to_string_lossy(), &cwd_str).inner()
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    DevOptions, DevWatchOptions, SharedClients, dev_context::DevContext, normalize_dev_options,
  };
  use futures::channel::mpsc::unbounded;
  use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions};
  use rolldown_fs_watcher::{PathsMut, WatcherBackend};
  use rolldown_workspace::TestDir;
  use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
  };
  use tokio::{
    sync::Notify,
    time::{Duration, timeout},
  };

  const LIVENESS_TIMEOUT: Duration = Duration::from_secs(10);

  fn is_registered(watcher: &StdMutex<FsWatcher>, path: &Path) -> bool {
    watcher.lock().expect("watcher lock").is_registered(path)
  }

  struct CommitFailingWatcher {
    commit_attempts: Arc<AtomicUsize>,
    failures_before_success: usize,
  }

  struct CommitFailingPaths {
    commit_attempts: Arc<AtomicUsize>,
    failures_before_success: usize,
    pending: Vec<PathBuf>,
  }

  impl PathsMut for CommitFailingPaths {
    fn add(&mut self, path: &Path) -> BuildResult<()> {
      self.pending.push(path.to_path_buf());
      Ok(())
    }

    fn commit(self: Box<Self>) -> BuildResult<()> {
      if self.pending.is_empty() {
        return Ok(());
      }
      if self.commit_attempts.fetch_add(1, Ordering::SeqCst) < self.failures_before_success {
        return Err(anyhow::anyhow!("intentional watcher commit failure").into());
      }
      Ok(())
    }
  }

  impl WatcherBackend for CommitFailingWatcher {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      Box::new(CommitFailingPaths {
        commit_attempts: Arc::clone(&self.commit_attempts),
        failures_before_success: self.failures_before_success,
        pending: Vec::new(),
      })
    }
  }

  struct AddFailingWatcher {
    commit_attempts: Arc<AtomicUsize>,
    fail_commit: bool,
  }

  struct AddFailingPaths {
    commit_attempts: Arc<AtomicUsize>,
    fail_commit: bool,
  }

  impl PathsMut for AddFailingPaths {
    fn add(&mut self, path: &Path) -> BuildResult<()> {
      if path.ends_with("fail.js") {
        return Err(anyhow::anyhow!("intentional watcher add failure").into());
      }
      Ok(())
    }

    fn commit(self: Box<Self>) -> BuildResult<()> {
      self.commit_attempts.fetch_add(1, Ordering::SeqCst);
      if self.fail_commit {
        return Err(anyhow::anyhow!("intentional watcher commit failure").into());
      }
      Ok(())
    }
  }

  impl WatcherBackend for AddFailingWatcher {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      Box::new(AddFailingPaths {
        commit_attempts: Arc::clone(&self.commit_attempts),
        fail_commit: self.fail_commit,
      })
    }
  }

  /// Records every path handed to the watcher so a test can assert what was
  /// actually registered, rather than what the coordinator intended.
  struct RecordingWatcher {
    added: Arc<StdMutex<Vec<PathBuf>>>,
  }

  struct RecordingPaths {
    added: Arc<StdMutex<Vec<PathBuf>>>,
    pending: Vec<PathBuf>,
  }

  impl PathsMut for RecordingPaths {
    fn add(&mut self, path: &Path) -> BuildResult<()> {
      self.pending.push(path.to_path_buf());
      Ok(())
    }

    fn commit(self: Box<Self>) -> BuildResult<()> {
      self.added.lock().expect("recording watcher lock").extend(self.pending);
      Ok(())
    }
  }

  impl WatcherBackend for RecordingWatcher {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      Box::new(RecordingPaths { added: Arc::clone(&self.added), pending: Vec::new() })
    }
  }

  #[test]
  fn failed_watch_add_commits_and_publishes_only_successful_additions() {
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher = FsWatcher::with_backend(Box::new(AddFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
      fail_commit: false,
    }));
    let watcher = StdMutex::new(watcher);
    let successful_before = ArcStr::from("/virtual/project/before.js");
    let failed = ArcStr::from("/virtual/project/fail.js");
    let successful_after = ArcStr::from("/virtual/project/after.js");
    let watch_files = [successful_before.clone(), failed.clone(), successful_after.clone()];

    BundleCoordinator::update_watch_paths_from(
      &watcher,
      &watch_files,
      Path::new("/virtual/project"),
      None,
      None,
    )
    .expect("a failed watcher addition must not fail the build");

    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);
    assert!(is_registered(&watcher, Path::new(successful_before.as_str())));
    assert!(!is_registered(&watcher, Path::new(failed.as_str())));
    assert!(is_registered(&watcher, Path::new(successful_after.as_str())));
  }

  #[test]
  fn refused_watch_add_is_skipped_and_commit_failure_is_returned_unpublished() {
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher = FsWatcher::with_backend(Box::new(AddFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
      fail_commit: true,
    }));
    let watcher = StdMutex::new(watcher);
    let successful_add = ArcStr::from("/virtual/project/success.js");
    let failed_add = ArcStr::from("/virtual/project/fail.js");
    let watch_files = [successful_add.clone(), failed_add.clone()];

    // The refused add is skipped by contract; only the commit failure is a
    // build error, and nothing is published when the commit fails.
    let error = BundleCoordinator::update_watch_paths_from(
      &watcher,
      &watch_files,
      Path::new("/virtual/project"),
      None,
      None,
    )
    .expect_err("the commit failure must be reported");
    let message = error.to_string();

    assert!(!message.contains("intentional watcher add failure"));
    assert!(message.contains("intentional watcher commit failure"));
    assert_eq!(error.len(), 1);
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);
    assert!(!is_registered(&watcher, Path::new(successful_add.as_str())));
    assert!(!is_registered(&watcher, Path::new(failed_add.as_str())));
  }

  #[test]
  fn failed_watch_commit_is_not_published_and_is_retried() {
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher = FsWatcher::with_backend(Box::new(CommitFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
      failures_before_success: 1,
    }));
    let watcher = StdMutex::new(watcher);
    let watch_file = ArcStr::from("/virtual/project/input.js");

    let first = BundleCoordinator::update_watch_paths_from(
      &watcher,
      std::slice::from_ref(&watch_file),
      Path::new("/virtual/project"),
      None,
      None,
    );
    assert!(first.is_err());
    assert!(!is_registered(&watcher, Path::new(watch_file.as_str())));
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);

    BundleCoordinator::update_watch_paths_from(
      &watcher,
      std::slice::from_ref(&watch_file),
      Path::new("/virtual/project"),
      None,
      None,
    )
    .expect("second watcher commit should retry and succeed");
    assert!(is_registered(&watcher, Path::new(watch_file.as_str())));
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 2);
  }

  /// `close()` drains the running task and reports its `onOutput` failure,
  /// even though the task's `BundleCompleted` is never processed.
  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn active_close_reports_the_running_task_callback_failure() {
    let test_dir = TestDir::new_canonical("rolldown-dev-watch-registration");
    let input = test_dir.path().join("main.js");
    fs::write(&input, "export const value = 1;").expect("write test input");

    let mut bundler = Bundler::new(BundlerOptions {
      cwd: Some(test_dir.path().to_path_buf()),
      input: Some(vec![input.to_string_lossy().into_owned().into()]),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        dev_mode: Some(DevModeOptions::default()),
        ..Default::default()
      }),
      ..Default::default()
    })
    .expect("create test bundler");
    bundler.generate().await.expect("generate initial bundle");

    let callback_entered = Arc::new(Notify::new());
    let callback_release = Arc::new(Notify::new());
    let callback_error: DevCallbackError =
      Arc::new(std::io::Error::other("intentional running-task callback failure"));
    let (coordinator_tx, coordinator_rx) = unbounded();
    let ctx = Arc::new(DevContext {
      options: normalize_dev_options(DevOptions {
        on_output: Some({
          let callback_entered = Arc::clone(&callback_entered);
          let callback_release = Arc::clone(&callback_release);
          let callback_error = Arc::clone(&callback_error);
          Arc::new(move |_| {
            let callback_entered = Arc::clone(&callback_entered);
            let callback_release = Arc::clone(&callback_release);
            let callback_error = Arc::clone(&callback_error);
            Box::pin(async move {
              callback_entered.notify_one();
              callback_release.notified().await;
              Err(callback_error)
            })
          })
        }),
        watch: Some(DevWatchOptions { skip_write: Some(true), ..Default::default() }),
        ..Default::default()
      }),
      coordinator_tx,
      clients: SharedClients::default(),
      stamp_table: Arc::new(Mutex::new(rolldown_common::HmrStampTable::default())),
      pending_payloads: Arc::new(Mutex::new(rustc_hash::FxHashMap::default())),
      top_level_evaluated: Mutex::new(Arc::new(rustc_hash::FxHashMap::default())),
      last_task_errored: std::sync::atomic::AtomicBool::new(false),
    });
    let mut coordinator = BundleCoordinator::new(
      Arc::new(Mutex::new(bundler)),
      ctx,
      coordinator_rx,
      FsWatcher::with_backend(Box::new(RecordingWatcher {
        added: Arc::new(StdMutex::new(Vec::new())),
      })),
      Arc::new(AtomicU32::new(0)),
    );

    coordinator.state = CoordinatorState::Idle;
    let mut changed_files = FxIndexMap::default();
    changed_files.insert(input.clone(), WatcherChangeKind::Update);
    coordinator.queued_tasks.push_back(TaskInput::Rebuild { changed_files });
    coordinator.schedule_build_if_stale().await.expect("schedule the final active build");

    timeout(LIVENESS_TIMEOUT, callback_entered.notified())
      .await
      .expect("active build callback must start before the liveness deadline");

    let release_callback = async {
      tokio::task::yield_now().await;
      callback_release.notify_one();
    };
    let (close_result, ()) = tokio::join!(coordinator.close(), release_callback);
    let close_error = close_result.expect_err("close must report the running task's callback");
    assert!(close_error.to_string().contains("intentional running-task callback failure"));
    assert_eq!(close_error.len(), 1);
  }

  #[test]
  fn close_aggregates_callback_and_close_failures() {
    let callback_error: DevCallbackError =
      Arc::new(std::io::Error::other("intentional callback failure"));
    let close_result: BuildResult<()> =
      Err(anyhow::anyhow!("intentional closeBundle failure").into());

    let error =
      BundleCoordinator::merge_callback_and_close_results(Err(callback_error), close_result)
        .expect_err("both lifecycle failures must be aggregated");
    let messages = error.into_vec().iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(messages.len(), 2);
    assert!(messages[0].contains("intentional callback failure"), "{messages:?}");
    assert!(messages[1].contains("intentional closeBundle failure"), "{messages:?}");
  }

  /// A module reached only through a dynamic import is added to
  /// `plugin_driver.watch_files` by `compile_lazy_entry` and by nothing else.
  /// That set lives on the current bundle handle, so a rebuild landing between
  /// the lazy compile and the coordinator's read replaces the handle and the
  /// path is gone before it is ever registered — permanently, since no later
  /// build reintroduces it. The snapshot carried on `ModuleChanged` has to
  /// survive exactly that interleaving, which this test performs deterministically
  /// instead of racing for it.
  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn module_changed_registers_snapshot_paths_dropped_by_a_handle_replacement() {
    let test_dir = TestDir::new_canonical("rolldown-dev-watch-registration");
    let input = test_dir.path().join("main.js");
    fs::write(&input, "export const value = 1;").expect("write test input");

    let mut bundler = Bundler::new(BundlerOptions {
      cwd: Some(test_dir.path().to_path_buf()),
      input: Some(vec![input.to_string_lossy().into_owned().into()]),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        dev_mode: Some(DevModeOptions::default()),
        ..Default::default()
      }),
      ..Default::default()
    })
    .expect("create test bundler");
    bundler.generate().await.expect("generate initial bundle");

    // Stand in for `compile_lazy_entry`, which records the lazily compiled
    // module's sources on the handle it just built with.
    let lazy = test_dir.path().join("lazy.css");
    fs::write(&lazy, ".lazy { color: teal; }").expect("write lazy source");
    let lazy = ArcStr::from(lazy.to_string_lossy().into_owned());
    bundler.watch_files().insert(lazy.clone());
    let snapshot = bundler.watch_files().iter().map(|path| path.clone()).collect::<Vec<_>>();
    assert!(snapshot.contains(&lazy), "the snapshot must capture the lazily added path");

    // Force the losing interleaving: a rebuild completes first and installs a
    // fresh handle, whose `watch_files` never held the lazy path.
    bundler.generate().await.expect("generate replacement bundle");
    assert!(
      !bundler.watch_files().contains(&lazy),
      "the replacement handle must not carry the lazily added path, \
       otherwise this test is not exercising the race"
    );
    let live = bundler.watch_files().iter().map(|path| path.clone()).collect::<Vec<_>>();
    assert!(!live.is_empty(), "the replacement handle must still watch its own sources");

    let added = Arc::new(StdMutex::new(Vec::new()));
    let (coordinator_tx, coordinator_rx) = unbounded();
    let ctx = Arc::new(DevContext {
      options: normalize_dev_options(DevOptions {
        watch: Some(DevWatchOptions { skip_write: Some(true), ..Default::default() }),
        ..Default::default()
      }),
      coordinator_tx,
      clients: SharedClients::default(),
      stamp_table: Arc::new(Mutex::new(rolldown_common::HmrStampTable::default())),
      pending_payloads: Arc::new(Mutex::new(rustc_hash::FxHashMap::default())),
      top_level_evaluated: Mutex::new(Arc::new(rustc_hash::FxHashMap::default())),
      last_task_errored: std::sync::atomic::AtomicBool::new(false),
    });
    let mut coordinator = BundleCoordinator::new(
      Arc::new(Mutex::new(bundler)),
      ctx,
      coordinator_rx,
      FsWatcher::with_backend(Box::new(RecordingWatcher { added: Arc::clone(&added) })),
      Arc::new(AtomicU32::new(0)),
    );
    coordinator.state = CoordinatorState::InProgress;
    coordinator.current_bundling_future = Some(BundlingFuture::new(async { Ok(()) }));

    coordinator.handle_module_changed(lazy.to_string(), &snapshot).await;

    let added = added.lock().expect("recording watcher lock").clone();
    assert!(
      added.contains(&PathBuf::from(lazy.as_str())),
      "the snapshot's path must be registered even though the live handle lost it: {added:?}"
    );
    // Union, not substitution: taking only the snapshot would drop everything
    // the replacement handle contributed after it was taken.
    for path in &live {
      assert!(
        added.contains(&PathBuf::from(path.as_str())),
        "the live handle's path {path} must still be registered: {added:?}"
      );
    }
  }

  /// An `HmrRebuild` task runs both stages against one bundler: the HMR stage
  /// records the modules it pulls in on the current handle *and* merges them
  /// into the scan cache, then the rebuild installs a fresh handle whose
  /// `watch_files` starts empty and whose partial rescan no longer refetches
  /// those now-cached modules. A new import is therefore on the outgoing handle
  /// and on no other, and the coordinator only reads the handle after the task
  /// released the lock — so it stays unwatched, and edits to it never reach a
  /// client. `HmrRebuild` occurs on the default `RebuildStrategy::Never` through
  /// the `ErrorStage::Rebuild` recovery branch of `handle_file_changes`.
  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn bundle_completed_registers_paths_dropped_by_the_rebuild_handle() {
    let test_dir = TestDir::new_canonical("rolldown-dev-watch-registration");
    let input = test_dir.path().join("main.js");
    fs::write(&input, "export const value = 1;").expect("write test input");

    let mut bundler = Bundler::new(BundlerOptions {
      cwd: Some(test_dir.path().to_path_buf()),
      input: Some(vec![input.to_string_lossy().into_owned().into()]),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        dev_mode: Some(DevModeOptions::default()),
        ..Default::default()
      }),
      ..Default::default()
    })
    .expect("create test bundler");
    bundler.generate().await.expect("generate initial bundle");

    // The edit introduces a module the initial build never saw, so only this
    // task can register it.
    let dependency = test_dir.path().join("dep.js");
    fs::write(&dependency, "export const dep = 1;").expect("write the new dependency");
    fs::write(&input, "import './dep.js';\nexport const value = 2;").expect("rewrite test input");

    let added = Arc::new(StdMutex::new(Vec::new()));
    let (coordinator_tx, coordinator_rx) = unbounded();
    let ctx = Arc::new(DevContext {
      options: normalize_dev_options(DevOptions {
        watch: Some(DevWatchOptions { skip_write: Some(true), ..Default::default() }),
        ..Default::default()
      }),
      coordinator_tx,
      clients: SharedClients::default(),
      stamp_table: Arc::new(Mutex::new(rolldown_common::HmrStampTable::default())),
      pending_payloads: Arc::new(Mutex::new(rustc_hash::FxHashMap::default())),
      top_level_evaluated: Mutex::new(Arc::new(rustc_hash::FxHashMap::default())),
      last_task_errored: std::sync::atomic::AtomicBool::new(false),
    });
    // A registered client is what makes the HMR stage compute an update at all.
    ctx
      .clients
      .lock()
      .await
      .insert("test-client".to_string(), crate::types::client_session::ClientSession::default());

    let bundler = Arc::new(Mutex::new(bundler));
    let mut coordinator = BundleCoordinator::new(
      Arc::clone(&bundler),
      Arc::clone(&ctx),
      coordinator_rx,
      FsWatcher::with_backend(Box::new(RecordingWatcher { added: Arc::clone(&added) })),
      Arc::new(AtomicU32::new(0)),
    );
    coordinator.state = CoordinatorState::InProgress;

    let mut changed_files = FxIndexMap::default();
    changed_files.insert(input.clone(), WatcherChangeKind::Update);
    BundlingTask::new(
      TaskInput::HmrRebuild { changed_files },
      Arc::clone(&bundler),
      ctx,
      Arc::new(AtomicU32::new(0)),
    )
    .run()
    .await
    .expect("the hmr-then-rebuild task must succeed");

    assert!(
      !bundler.lock().await.watch_files().contains(dependency.to_string_lossy().as_ref()),
      "the rebuild's handle must have dropped the new dependency, \
       otherwise this test is not exercising the hazard"
    );

    let completed = coordinator.rx.try_recv().expect("the task must report its completion");
    let CoordinatorMsg::BundleCompleted {
      error_stage,
      has_generated_bundle_output,
      callback_error,
      watch_files,
    } = completed
    else {
      panic!("a finished task must enqueue BundleCompleted");
    };
    coordinator
      .handle_bundle_completed(
        error_stage,
        has_generated_bundle_output,
        callback_error,
        &watch_files,
      )
      .await;

    let added = added.lock().expect("recording watcher lock").clone();
    assert!(
      added.contains(&dependency),
      "the module the HMR stage pulled in must be watched: {added:?}"
    );
    // Union, not substitution: the replacement handle's own sources stay watched.
    assert!(added.contains(&input), "the rescanned entry must still be watched: {added:?}");
  }
}
