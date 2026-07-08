use std::{
  collections::VecDeque,
  path::PathBuf,
  sync::{Arc, Mutex as StdMutex, atomic::AtomicU32},
};

use anyhow::Context;
use arcstr::ArcStr;
use notify::EventKind;
use rolldown_common::WatcherChangeKind;
use rolldown_dev_common::types::{DevCallbackError, DevCallbackResult};
use rolldown_error::{BatchedBuildDiagnostic, BuildResult};
use rolldown_fs_watcher::{DynFsWatcher, FsEventResult, RecursiveMode};
use rolldown_utils::{
  dashmap::FxDashSet, futures::spawn_detached, indexmap::FxIndexMap, pattern_filter,
};
use rustc_hash::FxHashSet;
use sugar_path::SugarPath;
use tokio::sync::Mutex;

use rolldown::Bundler;

use crate::{
  bundling_task::BundlingTask,
  dev_context::{
    BundlingFuture, RetainedDevCallbackErrors, SharedDevContext,
    dev_callback_result_to_build_result,
  },
  type_aliases::{CoordinatorReceiver, CoordinatorSender, WatchRegistrationErrorObserverId},
  types::{
    coordinator_msg::CoordinatorMsg, coordinator_state::CoordinatorState,
    coordinator_state_snapshot::CoordinatorStateSnapshot,
    ensure_latest_bundle_output_return::EnsureLatestBundleOutputReturn, error_stage::ErrorStage,
    schedule_build_return::ScheduleBuildReturn, task_input::TaskInput,
  },
  watcher_event_handler::WatcherEventHandler,
};

struct WatchRegistrationErrorEvent {
  error: DevCallbackError,
  pending_observers: FxHashSet<WatchRegistrationErrorObserverId>,
  recovered: bool,
  observed: bool,
}

/// BundleCoordinator - coordinates build tasks and manages initial build state
pub struct BundleCoordinator {
  bundler: Arc<Mutex<Bundler>>,
  ctx: SharedDevContext,
  next_hmr_patch_id: Arc<AtomicU32>,
  rx: CoordinatorReceiver,
  watcher: StdMutex<DynFsWatcher>,
  watched_files: FxDashSet<ArcStr>,
  /// Tracks the state of the initial build
  state: CoordinatorState,
  /// File changes that arrived during initial build
  queued_file_changes_waited_for_full_build: FxIndexMap<PathBuf, WatcherChangeKind>,
  /// Build state - managed directly by coordinator
  queued_tasks: VecDeque<TaskInput>,
  has_stale_bundle_output: bool,
  current_bundling_future: Option<BundlingFuture>,
  last_callback_error: Option<DevCallbackError>,
  active_watch_registration_error_observers: FxHashSet<WatchRegistrationErrorObserverId>,
  watch_registration_errors: VecDeque<WatchRegistrationErrorEvent>,
  next_watch_registration_error_observer_id: WatchRegistrationErrorObserverId,
}

impl BundleCoordinator {
  pub fn new(
    bundler: Arc<Mutex<Bundler>>,
    ctx: SharedDevContext,
    rx: CoordinatorReceiver,
    watcher: DynFsWatcher,
  ) -> Self {
    Self {
      bundler,
      ctx,
      next_hmr_patch_id: Arc::new(AtomicU32::new(0)),
      rx,
      watcher: StdMutex::new(watcher),
      watched_files: FxDashSet::default(),
      state: CoordinatorState::Initialized,
      queued_file_changes_waited_for_full_build: FxIndexMap::default(),
      // Initialize build state with initial build task
      queued_tasks: VecDeque::from([]),
      has_stale_bundle_output: true,
      current_bundling_future: None,
      last_callback_error: None,
      active_watch_registration_error_observers: FxHashSet::default(),
      watch_registration_errors: VecDeque::new(),
      next_watch_registration_error_observer_id: 1,
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
    while let Some(msg) = self.rx.recv().await {
      tracing::trace!("[BundleCoordinator] received message\n - message: {msg:#?}");
      match msg {
        CoordinatorMsg::WatchEvent(watch_event) => {
          self.handle_watch_event(watch_event).await;
        }
        CoordinatorMsg::BundleCompleted {
          error_stage,
          has_generated_bundle_output,
          callback_error,
        } => {
          self
            .handle_bundle_completed(error_stage, has_generated_bundle_output, callback_error)
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
        CoordinatorMsg::BeginWatchRegistrationErrorObservation { reply } => {
          let observer_id = self.begin_watch_registration_error_observation();
          let _ = reply.send(observer_id);
        }
        CoordinatorMsg::FinishWatchRegistrationErrorObservation { observer_id, reply } => {
          let error = self.finish_watch_registration_error_observation(observer_id);
          let _ = reply.send(error);
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
          let result = self.watched_files.iter().map(|s| s.to_string()).collect();
          let _ = reply.send(result);
        }
        CoordinatorMsg::ModuleChanged { module_id } => {
          self.handle_module_changed(module_id).await;
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
  async fn handle_module_changed(&mut self, module_id: String) {
    // `plugin_driver.watch_files` added in `bundler.compile_lazy_entry`
    // will be removed when task rebuild starts, so publish those paths first.
    // A failed publication must outlive the build that this message schedules.
    let watch_paths_result = self.update_watch_paths().await;

    let mut changed_files = FxIndexMap::default();
    changed_files.insert(PathBuf::from(&module_id), WatcherChangeKind::Update);

    self.queued_tasks.push_back(TaskInput::Rebuild { changed_files });
    self.has_stale_bundle_output = true;

    let _ = self.schedule_build_if_stale().await;
    self.retain_watch_registration_result(watch_paths_result);
  }

  async fn close(&mut self) -> BuildResult<()> {
    let watch_registration_error_observer = self.begin_watch_registration_error_observation();
    // A running task may replace `last_bundle_handle` after its HMR
    // stage. Wait for the complete task before closing the bundler so
    // `closeBundle` always runs on the final installed plugin driver.
    // See internal-docs/dev-engine/implementation.md.
    let callback_result = if let Some(bundling_future) = self.current_bundling_future.take() {
      let callback_result = bundling_future.await;
      // `BundlingTask::run` queued `BundleCompleted` before resolving, but the
      // coordinator cannot process that message while this close handler owns
      // the actor loop. Publish the final handle's watch paths here instead.
      let watch_paths_result = self.update_watch_paths().await;
      self.retain_watch_registration_result(watch_paths_result);
      callback_result
    } else if let Some(error) = self.last_callback_error.take() {
      Err(error)
    } else {
      Ok(())
    };
    let watch_registration_error =
      self.finish_watch_registration_error_observation(watch_registration_error_observer);
    let close_result = {
      let mut bundler = self.bundler.lock().await;
      bundler.close().await
    };
    Self::merge_callback_registration_and_close_results(
      callback_result,
      watch_registration_error,
      close_result,
    )
  }

  /// Handle file change events from watcher
  async fn handle_watch_event(&mut self, watch_event: FsEventResult) {
    match watch_event {
      Ok(batched_events) => {
        let mut changed_files = FxIndexMap::default();
        batched_events.into_iter().for_each(|batched_event| {
          match &batched_event.detail.kind {
            EventKind::Create(_create_kind) => {
              for path in batched_event.detail.paths {
                changed_files.insert(path, WatcherChangeKind::Create);
              }
            }
            #[cfg(target_os = "macos")]
            EventKind::Modify(notify::event::ModifyKind::Metadata(_))
              if !self.ctx.options.use_polling =>
            {
              // When using kqueue on mac, ignore metadata changes as it happens frequently and doesn't affect the build in most cases
              // Note that when using polling, we shouldn't ignore metadata changes as the polling watcher prefer to emit them over
              // content change events
            }
            EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::From))
            | EventKind::Remove(_) => {
              for path in batched_event.detail.paths {
                changed_files.insert(path, WatcherChangeKind::Delete);
              }
            }
            EventKind::Modify(_modify_kind) => {
              for path in batched_event.detail.paths {
                changed_files.insert(path, WatcherChangeKind::Update);
              }
            }
            _ => {}
          }
        });

        self.handle_file_changes(changed_files).await;
      }
      Err(e) => {
        tracing::error!("notify error: {e:?}");
      }
    }
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
  async fn handle_bundle_completed(
    &mut self,
    error_stage: Option<ErrorStage>,
    has_generated_bundle_output: bool,
    callback_error: Option<DevCallbackError>,
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
        let watch_paths_result = self.update_watch_paths().await;
        self.retain_watch_registration_result(watch_paths_result);

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
        let watch_paths_result = self.update_watch_paths().await;
        self.retain_watch_registration_result(watch_paths_result);

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

  fn merge_callback_registration_and_close_results(
    callback_result: DevCallbackResult,
    watch_registration_error: Option<DevCallbackError>,
    close_result: BuildResult<()>,
  ) -> BuildResult<()> {
    let callback_result = dev_callback_result_to_build_result(callback_result);
    let watch_registration_result = watch_registration_error
      .map_or_else(|| Ok(()), |error| dev_callback_result_to_build_result(Err(error)));
    let callback_and_registration_result =
      Self::merge_build_results(callback_result, watch_registration_result);
    Self::merge_build_results(callback_and_registration_result, close_result)
  }

  fn merge_build_results(primary: BuildResult<()>, secondary: BuildResult<()>) -> BuildResult<()> {
    match (primary, secondary) {
      (Ok(()), Ok(())) => Ok(()),
      (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
      (Err(primary_error), Err(secondary_error)) => {
        let mut errors = primary_error.into_vec();
        errors.extend(secondary_error.into_vec());
        Err(errors.into())
      }
    }
  }

  fn retain_watch_registration_result(&mut self, watch_paths_result: BuildResult<()>) {
    // Every publication attempt supersedes the previous failure occurrence.
    // Already-attached observers still receive older events, while a failed
    // retry becomes a new event for current and future observers.
    for error in &mut self.watch_registration_errors {
      error.recovered = true;
    }
    self.prune_acknowledged_watch_registration_errors();

    match watch_paths_result {
      Ok(()) => {}
      Err(error) => {
        self.watch_registration_errors.push_back(WatchRegistrationErrorEvent {
          error: Arc::new(error),
          pending_observers: self.active_watch_registration_error_observers.clone(),
          recovered: false,
          observed: false,
        });
      }
    }
  }

  fn begin_watch_registration_error_observation(&mut self) -> WatchRegistrationErrorObserverId {
    let observer_id = loop {
      let observer_id = self.next_watch_registration_error_observer_id;
      self.next_watch_registration_error_observer_id =
        self.next_watch_registration_error_observer_id.wrapping_add(1).max(1);
      if self.active_watch_registration_error_observers.insert(observer_id) {
        break observer_id;
      }
    };

    for error in &mut self.watch_registration_errors {
      if !error.recovered || !error.observed {
        error.pending_observers.insert(observer_id);
      }
    }

    observer_id
  }

  fn finish_watch_registration_error_observation(
    &mut self,
    observer_id: WatchRegistrationErrorObserverId,
  ) -> Option<DevCallbackError> {
    self.active_watch_registration_error_observers.remove(&observer_id);
    let mut observed_errors = Vec::new();
    for error in &mut self.watch_registration_errors {
      if error.pending_observers.remove(&observer_id) {
        error.observed = true;
        observed_errors.push(Arc::clone(&error.error));
      }
    }
    self.prune_acknowledged_watch_registration_errors();
    Self::merge_dev_callback_errors(observed_errors)
  }

  fn prune_acknowledged_watch_registration_errors(&mut self) {
    self
      .watch_registration_errors
      .retain(|error| !(error.recovered && error.observed && error.pending_observers.is_empty()));
  }

  fn merge_dev_callback_errors(errors: Vec<DevCallbackError>) -> Option<DevCallbackError> {
    let mut errors = errors.into_iter();
    let first = errors.next()?;
    let Some(second) = errors.next() else {
      return Some(first);
    };

    Some(RetainedDevCallbackErrors::into_error(
      std::iter::once(first).chain(std::iter::once(second)).chain(errors).collect(),
    ))
  }

  fn set_initial_build_state(&mut self, new_state: CoordinatorState) {
    self.state = new_state;
  }

  /// Update watcher paths based on current build output
  async fn update_watch_paths(&self) -> BuildResult<()> {
    let (watch_files, cwd) = {
      let bundler = self.bundler.lock().await;
      (
        bundler.watch_files().iter().map(|watch_file| watch_file.clone()).collect::<Vec<_>>(),
        bundler.options().cwd.to_string_lossy().into_owned(),
      )
    };

    let include = self.ctx.options.watch_include.as_deref();
    let exclude = self.ctx.options.watch_exclude.as_deref();

    Self::update_watch_paths_from(
      &self.watcher,
      &self.watched_files,
      &watch_files,
      &cwd,
      include,
      exclude,
    )
  }

  fn update_watch_paths_from(
    watcher: &StdMutex<DynFsWatcher>,
    watched_files: &FxDashSet<ArcStr>,
    watch_files: &[ArcStr],
    cwd: &str,
    include: Option<&[rolldown_utils::pattern_filter::StringOrRegex]>,
    exclude: Option<&[rolldown_utils::pattern_filter::StringOrRegex]>,
  ) -> BuildResult<()> {
    let mut watcher = watcher.lock().ok().context("Failed to acquire watcher lock")?;
    let mut paths_mut = watcher.paths_mut();
    let mut pending_watch_files = Vec::new();
    let mut add_errors = Vec::new();
    for watch_file in watch_files {
      let watch_file = &**watch_file;
      if !watched_files.contains(watch_file)
        && pattern_filter::filter(exclude, include, watch_file, cwd).inner()
      {
        match paths_mut.add(watch_file.as_path(), RecursiveMode::NonRecursive) {
          Ok(()) => pending_watch_files.push(ArcStr::from(watch_file)),
          Err(error) => add_errors.extend(error.into_vec()),
        }
      }
    }

    // Opening a notify paths transaction can pause event delivery until commit.
    // Always finalize it, including when one or more additions failed.
    // See internal-docs/dev-engine/implementation.md.
    let commit_result = paths_mut.commit();
    if commit_result.is_ok() {
      for watch_file in pending_watch_files {
        watched_files.insert(watch_file);
      }
    }

    let add_result =
      if add_errors.is_empty() { Ok(()) } else { Err(BatchedBuildDiagnostic::new(add_errors)) };
    Self::merge_build_results(add_result, commit_result)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    DevOptions, DevWatchOptions, SharedClients, dev_context::DevContext, normalize_dev_options,
  };
  use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions};
  use rolldown_fs_watcher::{FsEventHandler, FsWatcher, FsWatcherConfig, PathsMut};
  use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
  };
  use tokio::{
    sync::{Notify, mpsc::unbounded_channel},
    time::{Duration, timeout},
  };

  static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);
  const LIVENESS_TIMEOUT: Duration = Duration::from_secs(10);

  struct TestDir(PathBuf);

  impl TestDir {
    fn new() -> Self {
      let path = std::env::temp_dir().join(format!(
        "rolldown-dev-watch-registration-{}-{}",
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
    fn add(&mut self, path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
      self.pending.push(path.to_path_buf());
      Ok(())
    }

    fn remove(&mut self, _path: &Path) -> BuildResult<()> {
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

  impl FsWatcher for CommitFailingWatcher {
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

    fn watch(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
      unreachable!("test uses the batch path API")
    }

    fn unwatch(&mut self, _path: &Path) -> BuildResult<()> {
      unreachable!("test never removes paths")
    }

    fn paths_mut<'me>(&'me mut self) -> Box<dyn PathsMut + 'me> {
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
    fn add(&mut self, path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
      if path.ends_with("fail.js") {
        return Err(anyhow::anyhow!("intentional watcher add failure").into());
      }
      Ok(())
    }

    fn remove(&mut self, _path: &Path) -> BuildResult<()> {
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

  impl FsWatcher for AddFailingWatcher {
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

    fn watch(&mut self, _path: &Path, _recursive_mode: RecursiveMode) -> BuildResult<()> {
      unreachable!("test uses the batch path API")
    }

    fn unwatch(&mut self, _path: &Path) -> BuildResult<()> {
      unreachable!("test never removes paths")
    }

    fn paths_mut<'me>(&'me mut self) -> Box<dyn PathsMut + 'me> {
      Box::new(AddFailingPaths {
        commit_attempts: Arc::clone(&self.commit_attempts),
        fail_commit: self.fail_commit,
      })
    }
  }

  #[test]
  fn failed_watch_add_commits_and_publishes_only_successful_additions() {
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher: DynFsWatcher = Box::new(AddFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
      fail_commit: false,
    });
    let watcher = StdMutex::new(watcher);
    let watched_files = FxDashSet::default();
    let successful_before = ArcStr::from("/virtual/project/before.js");
    let failed = ArcStr::from("/virtual/project/fail.js");
    let successful_after = ArcStr::from("/virtual/project/after.js");
    let watch_files = [successful_before.clone(), failed.clone(), successful_after.clone()];

    let error = BundleCoordinator::update_watch_paths_from(
      &watcher,
      &watched_files,
      &watch_files,
      "/virtual/project",
      None,
      None,
    )
    .expect_err("the failed watcher addition must be reported");

    assert!(error.to_string().contains("intentional watcher add failure"));
    assert_eq!(error.len(), 1);
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);
    assert!(watched_files.contains(successful_before.as_str()));
    assert!(!watched_files.contains(failed.as_str()));
    assert!(watched_files.contains(successful_after.as_str()));
  }

  #[test]
  fn watch_add_and_commit_failures_are_aggregated_without_publication() {
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher: DynFsWatcher = Box::new(AddFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
      fail_commit: true,
    });
    let watcher = StdMutex::new(watcher);
    let watched_files = FxDashSet::default();
    let successful_add = ArcStr::from("/virtual/project/success.js");
    let failed_add = ArcStr::from("/virtual/project/fail.js");
    let watch_files = [successful_add.clone(), failed_add.clone()];

    let error = BundleCoordinator::update_watch_paths_from(
      &watcher,
      &watched_files,
      &watch_files,
      "/virtual/project",
      None,
      None,
    )
    .expect_err("add and commit failures must both be reported");
    let message = error.to_string();

    assert!(message.contains("intentional watcher add failure"));
    assert!(message.contains("intentional watcher commit failure"));
    assert_eq!(error.len(), 2);
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);
    assert!(!watched_files.contains(successful_add.as_str()));
    assert!(!watched_files.contains(failed_add.as_str()));
  }

  #[test]
  fn failed_watch_commit_is_not_published_and_is_retried() {
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher: DynFsWatcher = Box::new(CommitFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
      failures_before_success: 1,
    });
    let watcher = StdMutex::new(watcher);
    let watched_files = FxDashSet::default();
    let watch_file = ArcStr::from("/virtual/project/input.js");

    let first = BundleCoordinator::update_watch_paths_from(
      &watcher,
      &watched_files,
      std::slice::from_ref(&watch_file),
      "/virtual/project",
      None,
      None,
    );
    assert!(first.is_err());
    assert!(!watched_files.contains(watch_file.as_str()));
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);

    BundleCoordinator::update_watch_paths_from(
      &watcher,
      &watched_files,
      std::slice::from_ref(&watch_file),
      "/virtual/project",
      None,
      None,
    )
    .expect("second watcher commit should retry and succeed");
    assert!(watched_files.contains(watch_file.as_str()));
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 2);
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn recovered_module_registration_failure_reaches_queued_observers_once() {
    let test_dir = TestDir::new();
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write test input");

    let mut bundler = Bundler::new(BundlerOptions {
      cwd: Some(test_dir.0.clone()),
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
      Arc::new(std::io::Error::other("intentional queued-build callback failure"));
    let (coordinator_tx, coordinator_rx) = unbounded_channel();
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
    });
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher: DynFsWatcher = Box::new(CommitFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
      failures_before_success: 2,
    });
    let mut coordinator =
      BundleCoordinator::new(Arc::new(Mutex::new(bundler)), ctx, coordinator_rx, watcher);

    let first_observer = coordinator.begin_watch_registration_error_observation();
    let second_observer = coordinator.begin_watch_registration_error_observation();
    coordinator.state = CoordinatorState::InProgress;
    coordinator.current_bundling_future = Some(BundlingFuture::new(async { Ok(()) }));
    coordinator.handle_module_changed(input.to_string_lossy().into_owned()).await;

    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(coordinator.queued_tasks.len(), 1);
    assert_eq!(coordinator.watch_registration_errors.len(), 1);
    assert!(coordinator.watch_registration_errors[0].pending_observers.contains(&first_observer));
    assert!(coordinator.watch_registration_errors[0].pending_observers.contains(&second_observer));

    coordinator.handle_bundle_completed(None, true, None).await;
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(coordinator.state, CoordinatorState::InProgress);
    assert!(coordinator.current_bundling_future.is_some());
    assert!(coordinator.watch_registration_errors[0].recovered);
    assert!(!coordinator.watch_registration_errors[1].recovered);

    let first_error = coordinator
      .finish_watch_registration_error_observation(first_observer)
      .expect("first queued observer must receive both registration failures");
    assert!(first_error.to_string().contains("intentional watcher commit failure"));
    let first_error = dev_callback_result_to_build_result(Err(first_error))
      .expect_err("queued observer failures must remain diagnostics");
    assert_eq!(first_error.len(), 2);
    assert_eq!(coordinator.watch_registration_errors.len(), 2);

    let late_observer = coordinator.begin_watch_registration_error_observation();
    let late_error = coordinator
      .finish_watch_registration_error_observation(late_observer)
      .expect("late observer must receive only the current failed retry");
    let late_error = dev_callback_result_to_build_result(Err(late_error))
      .expect_err("the current failed retry must remain a diagnostic");
    assert_eq!(
      late_error.len(),
      1,
      "the superseded first failure must not poison later lifecycle calls"
    );

    let watch_paths_result = coordinator.update_watch_paths().await;
    coordinator.retain_watch_registration_result(watch_paths_result);
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 3);

    let second_error = coordinator
      .finish_watch_registration_error_observation(second_observer)
      .expect("second queued observer must receive both registration failures");
    assert!(second_error.to_string().contains("intentional watcher commit failure"));
    let second_error = dev_callback_result_to_build_result(Err(second_error))
      .expect_err("queued observer failures must remain diagnostics");
    assert_eq!(second_error.len(), 2);
    assert!(coordinator.watch_registration_errors.is_empty());

    let recovered_observer = coordinator.begin_watch_registration_error_observation();
    assert!(
      coordinator.finish_watch_registration_error_observation(recovered_observer).is_none(),
      "acknowledged failures must not poison observers after recovery"
    );

    timeout(LIVENESS_TIMEOUT, callback_entered.notified())
      .await
      .expect("queued build callback must start before the liveness deadline");

    let release_callback = async {
      tokio::task::yield_now().await;
      callback_release.notify_one();
    };
    let (close_result, ()) = tokio::join!(coordinator.close(), release_callback);
    let close_error = close_result.expect_err("close must report the queued callback failure");
    let message = close_error.to_string();
    assert!(message.contains("intentional queued-build callback failure"));
    assert!(!message.contains("intentional watcher commit failure"));
    assert_eq!(close_error.len(), 1);
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn active_close_retains_final_build_watch_registration_failure() {
    let test_dir = TestDir::new();
    let input = test_dir.0.join("main.js");
    fs::write(&input, "export const value = 1;").expect("write test input");

    let mut bundler = Bundler::new(BundlerOptions {
      cwd: Some(test_dir.0.clone()),
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
    let (coordinator_tx, coordinator_rx) = unbounded_channel();
    let ctx = Arc::new(DevContext {
      options: normalize_dev_options(DevOptions {
        on_output: Some({
          let callback_entered = Arc::clone(&callback_entered);
          let callback_release = Arc::clone(&callback_release);
          Arc::new(move |_| {
            let callback_entered = Arc::clone(&callback_entered);
            let callback_release = Arc::clone(&callback_release);
            Box::pin(async move {
              callback_entered.notify_one();
              callback_release.notified().await;
              Ok(())
            })
          })
        }),
        watch: Some(DevWatchOptions { skip_write: Some(true), ..Default::default() }),
        ..Default::default()
      }),
      coordinator_tx,
      clients: SharedClients::default(),
    });
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher: DynFsWatcher = Box::new(CommitFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
      failures_before_success: 1,
    });
    let mut coordinator =
      BundleCoordinator::new(Arc::new(Mutex::new(bundler)), ctx, coordinator_rx, watcher);

    coordinator.state = CoordinatorState::Idle;
    let mut changed_files = FxIndexMap::default();
    changed_files.insert(input.clone(), WatcherChangeKind::Update);
    coordinator.queued_tasks.push_back(TaskInput::Rebuild { changed_files });
    coordinator.schedule_build_if_stale().await.expect("schedule the final active build");

    assert!(coordinator.watch_registration_errors.is_empty());
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 0);
    timeout(LIVENESS_TIMEOUT, callback_entered.notified())
      .await
      .expect("active build callback must start before the liveness deadline");

    let release_callback = async {
      tokio::task::yield_now().await;
      callback_release.notify_one();
    };
    let (close_result, ()) = tokio::join!(coordinator.close(), release_callback);
    let close_error =
      close_result.expect_err("close must report final watch-path registration failure");
    assert!(close_error.to_string().contains("intentional watcher commit failure"));
    assert_eq!(close_error.len(), 1);
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);
    assert!(!coordinator.watched_files.contains(input.to_string_lossy().as_ref()));
  }

  #[test]
  fn close_aggregates_callback_registration_and_close_failures() {
    let callback_error: DevCallbackError =
      Arc::new(std::io::Error::other("intentional callback failure"));
    let registration_error: DevCallbackError =
      Arc::new(BatchedBuildDiagnostic::from(anyhow::anyhow!("intentional registration failure")));
    let close_result: BuildResult<()> =
      Err(anyhow::anyhow!("intentional closeBundle failure").into());

    let error = BundleCoordinator::merge_callback_registration_and_close_results(
      Err(callback_error),
      Some(registration_error),
      close_result,
    )
    .expect_err("all lifecycle failures must be aggregated");
    let message = error.to_string();
    assert!(message.contains("intentional callback failure"));
    assert!(message.contains("intentional registration failure"));
    assert!(message.contains("intentional closeBundle failure"));
    assert_eq!(error.len(), 3);
  }
}
