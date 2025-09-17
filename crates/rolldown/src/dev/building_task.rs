use std::{
  ops::Deref,
  path::PathBuf,
  sync::{Arc, atomic::AtomicU32},
  time::Duration,
};

use rolldown_common::ScanMode;
use rolldown_error::BuildResult;
use rolldown_utils::indexmap::FxIndexSet;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  dev::{build_driver_service::BuildMessage, dev_context::SharedDevContext},
  types::scan_stage_cache::ScanStageCache,
};

#[derive(Debug)]
pub struct TaskInput {
  pub changed_files: FxIndexSet<PathBuf>,
  pub require_full_rebuild: bool,
  pub generate_hmr_updates: bool,
  pub rebuild: bool,
}

impl TaskInput {
  pub fn new_initial_build_task() -> Self {
    Self {
      changed_files: FxIndexSet::default(),
      require_full_rebuild: true,
      rebuild: true,
      generate_hmr_updates: false,
    }
  }
}

pub struct BundlingTask {
  pub input: TaskInput,
  pub bundler: Arc<Mutex<Bundler>>,
  pub dev_context: SharedDevContext,
  pub bundler_cache: Option<ScanStageCache>,
  pub next_hmr_patch_id: Arc<AtomicU32>,
}

impl Deref for BundlingTask {
  type Target = TaskInput;

  fn deref(&self) -> &Self::Target {
    &self.input
  }
}

impl BundlingTask {
  pub async fn run(mut self) {
    if let Err(err) = self.run_inner().await {
      // FIXME: Should handle the error properly.
      eprintln!("Build error: {err}"); // FIXME: handle this error
      self.dev_context.state.lock().await.try_to_idle().expect("FIXME: Should not unwrap here");
    }

    if self.dev_context.build_channel_tx.send(BuildMessage::BuildFinish).is_err() {
      tracing::error!("Failed to send build finish message to build channel");
      // Notice: Send error will only happen when the channel is closed. It might be closed by an error or what.
      // Handling this error is helpful to that situation.
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    self.delay_to_merge_incoming_changes().await?;
    if self.generate_hmr_updates {
      self.generate_hmr_updates().await?;
    }
    if self.rebuild {
      self.rebuild().await?;
    }

    let mut build_state = self.dev_context.state.lock().await;
    build_state.cache = Some(self.bundler_cache.take().expect("Should never be none here"));
    build_state.try_to_idle()?;
    drop(build_state);
    Ok(())
  }

  async fn delay_to_merge_incoming_changes(&self) -> BuildResult<()> {
    let build_delay = 0;

    let mut build_status = if build_delay > 0 {
      loop {
        tokio::time::sleep(Duration::from_millis(build_delay)).await;
        let build_status = self.dev_context.state.lock().await;

        let has_no_changes = true; // TODO: implement delay merge behavior

        if has_no_changes {
          break build_status;
        }

        drop(build_status);
      }
    } else {
      self.dev_context.state.lock().await
    };

    tracing::trace!("`BuildStatus` is in building with changed files: {:#?}", self.changed_files);
    build_status.try_to_building()?;

    drop(build_status);

    Ok(())
  }

  pub async fn generate_hmr_updates(&mut self) -> BuildResult<()> {
    let bundler = self.bundler.lock().await;
    let bundler_cache = self.bundler_cache.take().expect("Should never be none here");
    let mut hmr_manager =
      bundler.create_hmr_manager(bundler_cache, Arc::clone(&self.next_hmr_patch_id));
    let changed_files =
      self.changed_files.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>();

    let updates = hmr_manager.compute_hmr_update_for_file_changes(&changed_files).await?;
    self.bundler_cache = Some(hmr_manager.input.cache);
    // We had updated the cache with those changes, so we can clear the changed files.
    // This way, we won't need to update the cache again in rebuild.
    if let Some(on_hmr_updates) = self.dev_context.options.on_hmr_updates.as_ref() {
      on_hmr_updates(updates, changed_files);
    }

    Ok(())
  }

  async fn rebuild(&mut self) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;

    if !self.require_full_rebuild {
      // We only need to pass the previous cache if it's an incremental rebuild.
      bundler.set_cache(self.bundler_cache.take().expect("Should never be none here"));
    }

    let skip_write = self.dev_context.options.skip_write;

    let scan_mode = if self.require_full_rebuild {
      ScanMode::Full
    } else {
      ScanMode::Partial(self.changed_files.iter().map(|p| p.to_string_lossy().into()).collect())
    };
    let scan_output = bundler.scan(scan_mode).await?;
    let _bundle_output = if skip_write {
      bundler.bundle_generate(scan_output).await
    } else {
      bundler.bundle_write(scan_output).await
    }?;

    tracing::trace!(
      "`BuildStatus` finished building with changed files: {:#?}",
      self.changed_files
    );
    self.bundler_cache = Some(bundler.take_cache());
    Ok(())
  }
}
