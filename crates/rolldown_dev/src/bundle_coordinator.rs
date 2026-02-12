use std::{
  collections::VecDeque,
  path::PathBuf,
  sync::{Arc, atomic::AtomicU32},
};

use arcstr::ArcStr;
use futures::FutureExt;
use notify::EventKind;
use rolldown_common::WatcherChangeKind;
use rolldown_error::BuildResult;
use rolldown_fs_watcher::{DynFsWatcher, FsEventResult, RecursiveMode};
use rolldown_utils::{dashmap::FxDashSet, indexmap::FxIndexMap};
use sugar_path::SugarPath;
use tokio::sync::Mutex;

use rolldown::Bundler;

use crate::{
  bundling_task::BundlingTask,
  dev_context::{BundlingFuture, PinBoxSendStaticFuture, SharedDevContext},
  type_aliases::{CoordinatorReceiver, CoordinatorSender},
  types::{
    coordinator_msg::CoordinatorMsg, coordinator_state::CoordinatorState,
    coordinator_state_snapshot::CoordinatorStateSnapshot,
    ensure_latest_bundle_output_return::EnsureLatestBundleOutputReturn,
    schedule_build_return::ScheduleBuildReturn, task_input::TaskInput,
  },
  watcher_event_handler::WatcherEventHandler,
};

/// BundleCoordinator - coordinates build tasks and manages initial build state
pub struct BundleCoordinator {
  bundler: Arc<Mutex<Bundler>>,
  ctx: SharedDevContext,
  next_hmr_patch_id: Arc<AtomicU32>,
  rx: CoordinatorReceiver,
  watcher: Mutex<DynFsWatcher>,
  watched_files: FxDashSet<ArcStr>,
  /// Tracks the state of the initial build
  state: CoordinatorState,
  /// File changes that arrived during initial build
  queued_file_changes_waited_for_full_build: FxIndexMap<PathBuf, WatcherChangeKind>,
  /// Build state - managed directly by coordinator
  queued_tasks: VecDeque<TaskInput>,
  has_stale_bundle_output: bool,
  current_bundling_future: Option<BundlingFuture>,
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
      watcher: Mutex::new(watcher),
      watched_files: FxDashSet::default(),
      state: CoordinatorState::Initialized,
      queued_file_changes_waited_for_full_build: FxIndexMap::default(),
      // Initialize build state with initial build task
      queued_tasks: VecDeque::from([]),
      has_stale_bundle_output: true,
      current_bundling_future: None,
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
        CoordinatorMsg::BundleCompleted { has_encountered_error, has_generated_bundle_output } => {
          self.handle_bundle_completed(has_encountered_error, has_generated_bundle_output).await;
        }
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
        CoordinatorMsg::GetWatchedFiles { reply } => {
          let result = self.watched_files.iter().map(|s| s.to_string()).collect();
          let _ = reply.send(result);
        }
        CoordinatorMsg::ModuleChanged { module_id } => {
          // Handle programmatic module change (e.g., lazy compilation executed)
          let mut changed_files = FxIndexMap::default();
          changed_files.insert(PathBuf::from(&module_id), WatcherChangeKind::Update);

          // Queue a rebuild task and mark output as stale
          self.queued_tasks.push_back(TaskInput::Rebuild { changed_files });
          self.has_stale_bundle_output = true;

          let _ = self.schedule_build_if_stale().await;
        }
        CoordinatorMsg::Close => {
          // Wait for any running bundling task to complete before exiting
          // to avoid the task panicking when it tries to send BundleCompleted
          if let Some(bundling_future) = self.current_bundling_future.take() {
            bundling_future.await;
          }
          break;
        }
      }
    }
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
        eprintln!("notify error: {e:?}");
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
      CoordinatorState::Idle | CoordinatorState::InProgress | CoordinatorState::Failed => {
        // The metal model for being `CoordinatorState::Failed` and receiving file changes is a bit of non-intuitive.
        // Like the file is edited 2 times, the first edit is invalid and the second edit fixes the error.
        // We just think the file is changed to second edit directly, ignoring the first invalid edit and follow the usual flow.
        let task_input = if self.ctx.options.rebuild_strategy.is_always() {
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
    has_encountered_error: bool,
    has_generated_bundle_output: bool,
  ) {
    match self.state {
      CoordinatorState::Initialized
      | CoordinatorState::Failed
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
        let _ = self.update_watch_paths().await;

        if has_encountered_error {
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

        if has_encountered_error {
          self.set_initial_build_state(CoordinatorState::Failed);
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
      CoordinatorState::Idle | CoordinatorState::FullBuildFailed | CoordinatorState::Failed => {
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
          let bundling_future = (Box::pin(bundling_task.run()) as PinBoxSendStaticFuture).shared();
          tokio::spawn(bundling_future.clone());

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
      CoordinatorState::FullBuildFailed | CoordinatorState::Failed => {
        // Clear all queued tasks and schedule a new full build
        self.queued_tasks.clear();
        self.queued_tasks.push_back(TaskInput::FullBuild);
        let schedule_result = self.schedule_build_if_stale().await;
        schedule_result.map(|ret| EnsureLatestBundleOutputReturn {
          future: ret.future,
          is_ensure_latest_bundle_output_future: true,
        })
      }
    }
  }

  /// Get current build status - atomic operation that doesn't block
  fn create_state_snapshot(&self) -> CoordinatorStateSnapshot {
    CoordinatorStateSnapshot {
      running_future: self.current_bundling_future.clone(),
      last_full_build_failed: self.state == CoordinatorState::FullBuildFailed,
      has_stale_output: self.has_stale_bundle_output,
    }
  }

  /// Set initial build state with logging
  fn set_initial_build_state(&mut self, new_state: CoordinatorState) {
    self.state = new_state;
  }

  /// Update watcher paths based on current build output
  async fn update_watch_paths(&self) -> BuildResult<()> {
    let bundler = self.bundler.lock().await;
    let watch_files = bundler.watch_files();

    let mut watcher = self.watcher.lock().await;
    let mut paths_mut = watcher.paths_mut();
    for watch_file in watch_files.iter() {
      let watch_file = &**watch_file;
      if !self.watched_files.contains(watch_file) {
        self.watched_files.insert(watch_file.to_string().into());
        paths_mut.add(watch_file.as_path(), RecursiveMode::NonRecursive)?;
      }
    }
    paths_mut.commit()?;
    Ok(())
  }
}
