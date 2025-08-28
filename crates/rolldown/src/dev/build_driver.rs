use std::{mem, path::PathBuf, sync::Arc};

use futures::FutureExt;

use rolldown_error::BuildResult;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  dev::{
    bundling_task::BundlingTask,
    dev_context::{BuildProcessFuture, PinBoxSendStaticFuture, SharedDevContext},
  },
};

pub type SharedBuildDriver = Arc<BuildDriver>;

pub struct BuildDriver {
  pub bundler: Arc<Mutex<Bundler>>,
  pub ctx: SharedDevContext,
}

impl BuildDriver {
  pub fn new(bundler: Arc<Mutex<Bundler>>, ctx: SharedDevContext) -> Self {
    Self { bundler, ctx }
  }

  pub async fn register_changed_files(&self, paths: Vec<PathBuf>) {
    tracing::trace!("Register changed files: {:?}", paths);
    let mut build_state = self.ctx.state.lock().await;
    build_state.changed_files.extend(paths);
  }

  /// Schedule a build to consume pending changed files.
  pub async fn schedule_build_if_stale(&self) -> BuildResult<Option<BuildProcessFuture>> {
    tracing::trace!("Start scheduling a build to consume pending changed files");
    let mut build_state = self.ctx.state.lock().await;
    tracing::trace!("Start scheduling a build to consume pending changed files2");
    if let Some(building_future) = build_state.is_busy_then_future().cloned() {
      tracing::trace!("A build is running, return the future immediately");
      drop(build_state);
      // If there's build running, it will be responsible to handle new changed files.
      // So, we only need to wait for the latest build to finish.
      Ok(Some(building_future))
    } else if build_state.require_full_rebuild || !build_state.changed_files.is_empty() {
      tracing::trace!(
        "Schedule a build to consume pending changed files due to {:?} or {:?}",
        build_state.require_full_rebuild,
        build_state.changed_files
      );
      // Note: Full rebuild and incremental build both clear changed files.
      let changed_files = mem::take(&mut build_state.changed_files);

      let bundling_task = BundlingTask {
        bundler: Arc::clone(&self.bundler),
        changed_files,
        require_full_rebuild: build_state.require_full_rebuild,
        dev_data: Arc::clone(&self.ctx),
        ensure_latest_build: true,
      };

      let bundling_future = (Box::pin(bundling_task.exec()) as PinBoxSendStaticFuture).shared();
      tokio::spawn(bundling_future.clone());

      build_state.try_to_delaying(bundling_future.clone())?;
      drop(build_state);

      Ok(Some(bundling_future))
    } else {
      tracing::trace!(
        "Nothing to do due to {:?} or {:?}",
        build_state.require_full_rebuild,
        build_state.changed_files
      );
      Ok(None)
    }
  }

  pub async fn ensure_latest_build(&self) -> BuildResult<()> {
    if let Some(future) = self.schedule_build_if_stale().await? {
      future.await;
    }
    Ok(())
  }
}
