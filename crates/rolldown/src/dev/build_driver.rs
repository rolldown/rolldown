use std::{mem, ops::Deref, path::PathBuf, sync::Arc};

use futures::FutureExt;

use tokio::sync::Mutex;

use crate::{
  Bundler, BundlerBuilder,
  dev::{
    bundling_task::BundlingTask,
    dev_context::{BuildProcessFuture, DevContext, PinBoxSendStaticFuture, SharedDevContext},
  },
};

pub type SharedBuildDriver = Arc<BuildDriver>;

pub struct BuildDriver {
  pub bundler: Arc<Mutex<Bundler>>,
  pub ctx: SharedDevContext,
}

impl BuildDriver {
  pub fn new(bundler_builder: BundlerBuilder) -> Self {
    let bundler = Arc::new(Mutex::new(bundler_builder.build()));
    let ctx = Arc::new(DevContext::default());

    Self { bundler, ctx }
  }

  pub async fn schedule_build(&self, changed_paths: Vec<PathBuf>) -> Option<BuildProcessFuture> {
    let mut build_status = self.ctx.status.lock().await;
    if build_status.is_in_building || build_status.is_in_debouncing {
      tracing::trace!(
        "Bailout due to is_in_building({}) or is_in_debouncing({}) with changed files: {:#?}",
        build_status.is_in_building,
        build_status.is_in_debouncing,
        build_status.changed_files,
      );
      build_status.changed_files.extend(changed_paths);
      return None;
    }

    let mut batched_changed_files = mem::take(&mut build_status.changed_files);
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
    build_status.is_in_debouncing = true;
    build_status.future = bundling_future.clone();
    drop(build_status);

    Some(bundling_future)
  }
}

impl Deref for BuildDriver {
  type Target = DevContext;

  fn deref(&self) -> &Self::Target {
    &self.ctx
  }
}
