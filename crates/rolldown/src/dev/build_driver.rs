use std::{
  path::PathBuf,
  sync::{Arc, atomic::AtomicU32},
};

use futures::FutureExt;

use rolldown_common::ClientHmrUpdate;
use rolldown_error::BuildResult;
use rolldown_utils::indexmap::FxIndexSet;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  dev::{
    building_task::BundlingTask,
    dev_context::{BuildProcessFuture, PinBoxSendStaticFuture, SharedDevContext},
    types::task_input::TaskInput,
  },
};

pub type SharedBuildDriver = Arc<BuildDriver>;

pub struct BuildDriver {
  pub bundler: Arc<Mutex<Bundler>>,
  pub ctx: SharedDevContext,
  next_hmr_patch_id: Arc<AtomicU32>,
}

impl BuildDriver {
  pub fn new(bundler: Arc<Mutex<Bundler>>, ctx: SharedDevContext) -> Self {
    Self { bundler, ctx, next_hmr_patch_id: Arc::new(AtomicU32::new(0)) }
  }

  pub async fn handle_file_changes(&self, changed_files: FxIndexSet<PathBuf>) {
    let task_input = if self.ctx.options.rebuild_strategy.is_always() {
      TaskInput::HmrRebuild { changed_files }
    } else {
      TaskInput::Hmr { changed_files }
    };
    let mut build_state = self.ctx.state.lock().await;
    build_state.queued_tasks.push_back(task_input);
  }

  /// Schedule a build to consume pending changed files.
  pub async fn schedule_build_if_stale(
    &self,
  ) -> BuildResult<Option<(BuildProcessFuture, /* already scheduled */ bool)>> {
    tracing::trace!("Calling `schedule_build_if_stale`");
    let mut build_state = self.ctx.state.lock().await;
    if let Some(building_future) = build_state.is_busy_then_future().cloned() {
      tracing::trace!("A build is running, return the future immediately");

      drop(build_state);
      // If there's build running, it will be responsible to handle new changed files.
      // So, we only need to wait for the latest build to finish.
      Ok(Some((building_future, true)))
    } else if let Some(mut task_input) = build_state.queued_tasks.pop_front() {
      tracing::trace!(
        "Schedule a build to consume pending changed files due to task{task_input:#?}",
      );

      // Merge mergeable task inputs into one.
      while let Some(peeked) = build_state.queued_tasks.pop_front() {
        if task_input.is_mergeable_with(&peeked) {
          task_input.merge_with(peeked);
        } else {
          build_state.queued_tasks.push_front(peeked);
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

      build_state.try_to_delaying(bundling_future.clone())?;
      drop(build_state);

      Ok(Some((bundling_future, false)))
    } else {
      tracing::trace!("Nothing to do due to no task in queue",);
      Ok(None)
    }
  }

  pub async fn has_latest_build_output(&self) -> bool {
    let build_state = self.ctx.state.lock().await;
    !build_state.has_stale_build_output
  }

  pub async fn ensure_latest_build_output(&self) -> BuildResult<()> {
    let mut count = 0;

    loop {
      count += 1;
      if count > 1000 {
        eprintln!(
          "Debug: `ensure_latest_build_output` wait for 1000 times build, something might be wrong"
        );
        break;
      }

      let mut build_state = self.ctx.state.lock().await;
      if let Some(building_future) = build_state.is_busy_then_future().cloned() {
        drop(build_state);
        building_future.await;
      } else {
        if build_state.has_stale_build_output && build_state.queued_tasks.is_empty() {
          build_state
            .queued_tasks
            .push_back(TaskInput::Rebuild { changed_files: FxIndexSet::default() });
        }
        drop(build_state);
        if let Some((building_future, _)) = self.schedule_build_if_stale().await? {
          building_future.await;
        } else {
          break;
        }
      }
    }

    Ok(())
  }

  pub async fn invalidate(
    &self,
    caller: String,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    let mut updates = Vec::new();
    for client in self.ctx.clients.iter() {
      loop {
        let build_state = self.ctx.state.lock().await;
        if let Some(building_future) = build_state.is_busy_then_future().cloned() {
          drop(build_state);
          building_future.await;
        } else {
          break;
        }
      }

      let mut bundler = self.bundler.lock().await;
      let update = bundler
        .compute_update_for_calling_invalidate(
          caller.clone(),
          first_invalidated_by.clone(),
          client.key(),
          &client.executed_modules,
          Arc::clone(&self.next_hmr_patch_id),
        )
        .await?;
      updates.push(ClientHmrUpdate { client_id: client.key().to_string(), update });
    }

    Ok(updates)
  }
}
