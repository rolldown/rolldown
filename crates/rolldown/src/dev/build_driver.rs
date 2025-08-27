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

  pub async fn schedule_build(
    &self,
    changed_paths: Vec<PathBuf>,
  ) -> BuildResult<Option<BuildProcessFuture>> {
    let mut build_state = self.ctx.status.lock().await;
    if build_state.is_busy() {
      tracing::trace!(
        "Bailout due to building({}) or delaying({}) with changed files: {:#?}",
        build_state.is_building(),
        build_state.is_delaying(),
        build_state.changed_files,
      );
      build_state.changed_files.extend(changed_paths);
      return Ok(None);
    }

    let mut batched_changed_files = mem::take(&mut build_state.changed_files);
    batched_changed_files.extend(changed_paths);

    let bundling_task = BundlingTask {
      bundler: Arc::clone(&self.bundler),
      changed_files: batched_changed_files,
      dev_data: Arc::clone(&self.ctx),
      ensure_latest_build: true,
    };

    let bundling_future = (Box::pin(bundling_task.exec()) as PinBoxSendStaticFuture).shared();
    tokio::spawn(bundling_future.clone());

    tracing::trace!("BuildStatus is in debouncing");
    build_state.try_to_delaying(bundling_future.clone())?;
    drop(build_state);

    Ok(Some(bundling_future))
  }
}
