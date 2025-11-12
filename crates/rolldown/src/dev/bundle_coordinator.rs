use std::{
  collections::VecDeque,
  path::PathBuf,
  sync::{Arc, atomic::AtomicU32},
};

use arcstr::ArcStr;
use futures::FutureExt;
use rolldown_error::BuildResult;
use rolldown_fs_watcher::{DynFsWatcher, FileChangeResult};
use rolldown_utils::{dashmap::FxDashSet, indexmap::FxIndexSet};
use sugar_path::SugarPath;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  dev::{
    bundling_task::BundlingTask,
    dev_context::{BuildProcessFuture, PinBoxSendStaticFuture, SharedDevContext},
    type_aliases::{CoordinatorReceiver, CoordinatorSender},
    types::{
      bundling_status::BundlingStatus, coordinator_msg::CoordinatorMsg,
      initial_build_state::InitialBuildState, task_input::TaskInput,
    },
    watcher_event_handler::WatcherEventHandler,
  },
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
  initial_build_state: InitialBuildState,
  /// File changes that arrived during initial build
  queued_file_changes: FxIndexSet<PathBuf>,
  /// Build state - managed directly by coordinator
  queued_tasks: VecDeque<TaskInput>,
  has_stale_build_output: bool,
  current_build: Option<BuildProcessFuture>,
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
      // Start in InProgress since we queue the initial build task immediately
      initial_build_state: InitialBuildState::InProgress,
      queued_file_changes: FxIndexSet::default(),
      // Initialize build state with initial build task
      queued_tasks: VecDeque::from([TaskInput::new_initial_build_task()]),
      has_stale_build_output: true,
      current_build: None,
    }
  }

  /// Create a watcher event handler that sends file change events to this coordinator
  pub fn create_watcher_event_handler(coordinator_tx: CoordinatorSender) -> WatcherEventHandler {
    WatcherEventHandler { coordinator_tx }
  }

  /// Run the coordinator message loop
  pub async fn run(mut self) {
    while let Some(msg) = {
      tracing::trace!("`BundleCoordinator` is waiting for messages.");
      self.rx.recv().await
    } {
      match msg {
        CoordinatorMsg::WatchEvent(watch_event) => {
          self.handle_watch_event(watch_event).await;
        }
        CoordinatorMsg::BuildCompleted { result, task_required_rebuild } => {
          self.handle_build_completed(result, task_required_rebuild).await;
        }
        CoordinatorMsg::ScheduleBuild { reply } => {
          let result = self.schedule_build_if_stale().await;
          let _ = reply.send(result);
        }
        CoordinatorMsg::HasLatestBuildOutput { reply } => {
          let has_latest = self.has_latest_build_output();
          let _ = reply.send(has_latest);
        }
        CoordinatorMsg::GetBuildStatus { reply } => {
          let status = self.get_build_status();
          let _ = reply.send(status);
        }
        CoordinatorMsg::EnsureCurrentBuildFinish { reply } => {
          if let Some(building_future) = self.current_build.clone() {
            building_future.await;
          }
          let _ = reply.send(());
        }
        CoordinatorMsg::Close => {
          break;
        }
      }
    }
  }

  /// Handle file change events from watcher
  async fn handle_watch_event(&mut self, watch_event: FileChangeResult) {
    match watch_event {
      Ok(batched_events) => {
        tracing::debug!(target: "hmr", "Received batched events: {:#?}", batched_events);

        let mut changed_files = FxIndexSet::default();
        batched_events.into_iter().for_each(|batched_event| match &batched_event.detail.kind {
          #[cfg(target_os = "macos")]
          notify::EventKind::Modify(notify::event::ModifyKind::Metadata(_))
            if !self.ctx.options.use_polling =>
          {
            // When using kqueue on mac, ignore metadata changes as it happens frequently and doesn't affect the build in most cases
            // Note that when using polling, we shouldn't ignore metadata changes as the polling watcher prefer to emit them over
            // content change events
          }
          notify::EventKind::Modify(_modify_kind) => {
            changed_files.extend(batched_event.detail.paths);
          }
          _ => {}
        });

        self.handle_file_changes(changed_files).await;
      }
      Err(e) => {
        eprintln!("notify error: {e:?}");
      }
    }
  }

  /// Handle file changes based on initial build state
  async fn handle_file_changes(&mut self, changed_files: FxIndexSet<PathBuf>) {
    if changed_files.is_empty() {
      return;
    }

    match self.initial_build_state {
      // If initial build in progress, queue the file changes
      InitialBuildState::InProgress => {
        tracing::debug!(
          "Initial build is {:?}, queuing {} file changes",
          self.initial_build_state,
          changed_files.len()
        );
        self.queued_file_changes.extend(changed_files);
      }

      // If initial build succeeded, map to Hmr or Rebuild task based on strategy
      InitialBuildState::Succeeded => {
        let task_input = if self.ctx.options.rebuild_strategy.is_always() {
          TaskInput::HmrRebuild { changed_files }
        } else {
          TaskInput::Hmr { changed_files }
        };

        self.queued_tasks.push_back(task_input);

        // Schedule build immediately
        let _ = self.schedule_build_if_stale().await;
      }

      // If initial build failed, create FullRebuild task and clear queue
      InitialBuildState::Failed => {
        tracing::info!(
          "Initial build failed, scheduling retry due to {:?}. Clearing {} queued changes.",
          changed_files,
          self.queued_file_changes.len()
        );

        // Clear the queued file changes - they'll be picked up by the full rebuild
        self.queued_file_changes.clear();

        // Queue a FullRebuild task to retry
        self.queued_tasks.push_back(TaskInput::FullRebuild);
        // Schedule build immediately
        // Reset to InProgress since we're retrying
        let schedule_result = self.schedule_build_if_stale().await;
        if let Ok(Some((_, already_scheduled))) = &schedule_result {
          if *already_scheduled {
            tracing::info!("A build is already scheduled to retry the initial build.");
          } else {
            self.set_initial_build_state(InitialBuildState::InProgress);
            tracing::info!("Scheduled a build to retry the initial build.");
          }
        } else if let Err(e) = &schedule_result {
          tracing::error!("Failed to schedule build to retry initial build: {:?}", e);
        }
      }
    }
  }

  /// Handle build completion notification
  async fn handle_build_completed(&mut self, result: BuildResult<()>, task_required_rebuild: bool) {
    tracing::trace!("BundleCoordinator received BuildCompleted: {:?}", result.is_ok());

    // Clear current build
    self.current_build = None;

    // Update has_stale_build_output based on task type and result
    if result.is_ok() {
      // Output is fresh if task included a rebuild
      self.has_stale_build_output = !task_required_rebuild;
    } else {
      // Output is stale if build failed
      self.has_stale_build_output = true;
    }

    match self.initial_build_state {
      InitialBuildState::InProgress => {
        if result.is_ok() {
          // Initial build succeeded!
          tracing::info!("Initial build succeeded");
          self.set_initial_build_state(InitialBuildState::Succeeded);

          // Update watch paths after initial build succeeds
          if let Err(e) = self.update_watch_paths().await {
            tracing::error!("Failed to update watch paths: {:?}", e);
          }

          // Process any queued file changes
          if !self.queued_file_changes.is_empty() {
            let queued_changes = std::mem::take(&mut self.queued_file_changes);
            tracing::info!(
              "Processing {} file changes that arrived during initial build",
              queued_changes.len()
            );
            self.handle_file_changes(queued_changes).await;
          }
        } else {
          // Initial build failed
          tracing::error!("Initial build failed");
          self.set_initial_build_state(InitialBuildState::Failed);
          // Keep queued file changes - they'll trigger retry when next change comes
        }
      }

      InitialBuildState::Succeeded | InitialBuildState::Failed => {
        // Incremental builds don't change initial build state
        tracing::trace!("Incremental build completed");
      }
    }

    // Schedule next build if there are queued tasks
    let _ = self.schedule_build_if_stale().await;
  }

  /// Schedule a build to consume pending changed files
  #[expect(clippy::unused_async)]
  async fn schedule_build_if_stale(
    &mut self,
  ) -> BuildResult<Option<(BuildProcessFuture, /* already scheduled */ bool)>> {
    tracing::trace!("Calling `schedule_build_if_stale`");
    if let Some(building_future) = self.current_build.clone() {
      tracing::trace!("A build is running, return the future immediately");

      // If there's build running, it will be responsible to handle new changed files.
      // So, we only need to wait for the latest build to finish.
      Ok(Some((building_future, true)))
    } else if let Some(mut task_input) = self.queued_tasks.pop_front() {
      tracing::trace!(
        "Schedule a build to consume pending changed files due to task{task_input:#?}",
      );

      // Merge mergeable task inputs into one.
      while let Some(peeked) = self.queued_tasks.pop_front() {
        if task_input.is_mergeable_with(&peeked) {
          task_input.merge_with(peeked);
        } else {
          self.queued_tasks.push_front(peeked);
          break;
        }
      }

      let bundling_task = BundlingTask {
        input: task_input,
        bundler: Arc::clone(&self.bundler),
        dev_context: Arc::clone(&self.ctx),
        next_hmr_patch_id: Arc::clone(&self.next_hmr_patch_id),
      };

      let bundling_future = (Box::pin(bundling_task.run()) as PinBoxSendStaticFuture).shared();
      tokio::spawn(bundling_future.clone());

      self.current_build = Some(bundling_future.clone());

      Ok(Some((bundling_future, false)))
    } else {
      tracing::trace!("Nothing to do due to no task in queue; {:?}", self.initial_build_state);
      Ok(None)
    }
  }

  fn has_latest_build_output(&self) -> bool {
    !self.has_stale_build_output
  }

  /// Get current build status - atomic operation that doesn't block
  fn get_build_status(&self) -> BundlingStatus {
    BundlingStatus {
      current_build_future: self.current_build.clone(),
      has_stale_output: self.has_stale_build_output,
      initial_build_state: self.initial_build_state,
    }
  }

  /// Set initial build state with logging
  fn set_initial_build_state(&mut self, new_state: InitialBuildState) {
    if self.initial_build_state != new_state {
      tracing::debug!(
        "Initial build state transition: {:?} -> {:?}",
        self.initial_build_state,
        new_state
      );
      self.initial_build_state = new_state;
    }
  }

  /// Update watcher paths based on current build output
  async fn update_watch_paths(&self) -> BuildResult<()> {
    let bundler = self.bundler.lock().await;
    let watch_files = bundler.watch_files();

    let mut watcher = self.watcher.lock().await;
    let mut paths_mut = watcher.paths_mut();
    for watch_file in watch_files.iter() {
      let watch_file = &**watch_file;
      tracing::trace!("watch file: {:?}", watch_file);
      if !self.watched_files.contains(watch_file) {
        self.watched_files.insert(watch_file.to_string().into());
        paths_mut.add(watch_file.as_path(), notify::RecursiveMode::NonRecursive)?;
      }
    }
    paths_mut.commit()?;
    Ok(())
  }
}
