use std::{
  ops::{Deref, DerefMut},
  sync::{Arc, atomic::AtomicU32},
  time::Duration,
};

use rolldown_common::{ClientHmrInput, ScanMode, WatcherChangeKind};
use rolldown_error::BuildResult;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  dev::{
    build_driver_service::BuildMessage, dev_context::SharedDevContext, types::task_input::TaskInput,
  },
  types::scan_stage_cache::ScanStageCache,
};

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

impl DerefMut for BundlingTask {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.input
  }
}

impl BundlingTask {
  pub async fn run(mut self) {
    tracing::trace!("Start running bundling task: {:#?}", self.input);
    if let Err(err) = self.run_inner().await {
      // FIXME: Should handle the error properly.
      eprintln!("Build error: {err}"); // FIXME: handle this error
    }

    let mut build_state = self.dev_context.state.lock().await;
    build_state.cache = Some(self.bundler_cache.take().expect("Should never be none here"));
    if let Err(err) = build_state.try_to_idle() {
      eprintln!("TODO: should handle this error {err:#?}");
      build_state.reset_to_idle();
    }
    build_state.has_stale_build_output = !self.input.requires_rebuild();
    drop(build_state);

    self.dev_context.build_channel_tx.send(BuildMessage::BuildFinish).expect(
      "Build service channel closed while sending BuildFinish - build service terminated unexpectedly"
    );
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    self.delay_to_merge_incoming_changes().await?;

    {
      let bundler = self.bundler.lock().await;
      for changed_file in self.input.changed_files() {
        bundler
          .plugin_driver
          // FIXME: use proper WatcherChangeKind for created/removed files.
          .watch_change(changed_file.to_str().unwrap(), WatcherChangeKind::Update)
          .await?;
      }
    }

    let mut has_full_reload_update = false;
    if self.input.require_generate_hmr_update() {
      self.generate_hmr_updates(&mut has_full_reload_update).await?;
    }

    // If the rebuild strategy is auto and there's a full reload update, we need to rebuild.
    // Convert Hmr to HmrRebuild if needed
    if self.dev_context.options.rebuild_strategy.is_auto()
      && has_full_reload_update
      && !self.input.requires_rebuild()
    {
      if let Some(changed_files) = self.input.changed_files_mut() {
        self.input = TaskInput::HmrRebuild { changed_files: std::mem::take(changed_files) };
      }
    }

    if self.input.requires_rebuild() {
      self.rebuild().await?;
    }

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

    tracing::trace!(
      "`BuildStatus` is in building with changed files: {:#?}",
      self.input.changed_files()
    );
    build_status.try_to_building()?;

    drop(build_status);

    Ok(())
  }

  pub async fn generate_hmr_updates(
    &mut self,
    has_full_reload_update: &mut bool,
  ) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;
    bundler.set_cache(self.bundler_cache.take().expect("Should never be none here"));
    let changed_files = self
      .input
      .changed_files()
      .iter()
      .map(|p| p.to_string_lossy().to_string())
      .collect::<Vec<_>>();

    // Build ClientHmrInput for each client
    // Store client sessions to keep data alive during HMR computation
    let client_sessions: Vec<_> = self.dev_context.clients.iter().collect();

    let client_inputs: Vec<ClientHmrInput> = client_sessions
      .iter()
      .map(|client| ClientHmrInput {
        client_id: client.key(),
        executed_modules: &client.executed_modules,
      })
      .collect();

    // Compute HMR updates for all clients in one call
    let hmr_result = bundler
      .compute_hmr_update_for_file_changes(
        &changed_files,
        &client_inputs,
        Arc::clone(&self.next_hmr_patch_id),
      )
      .await;

    // Check if any update is a full reload (only if successful)
    if let Ok(client_updates) = &hmr_result {
      for update in client_updates {
        if update.update.is_full_reload() {
          *has_full_reload_update = true;
        }
      }
    }

    self.bundler_cache = Some(bundler.take_cache());

    // Call on_hmr_updates callback if provided
    if let Some(on_hmr_updates) = self.dev_context.options.on_hmr_updates.as_ref() {
      match hmr_result {
        Ok(client_updates) => {
          on_hmr_updates(Ok((client_updates, changed_files)));
        }
        Err(e) => {
          on_hmr_updates(Err(e));
        }
      }
      Ok(())
    } else {
      hmr_result.map(|_| ())
    }
  }

  async fn rebuild(&mut self) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;

    if !self.input.requires_full_rebuild() {
      // We only need to pass the previous cache if it's an incremental rebuild.
      bundler.set_cache(self.bundler_cache.take().expect("Should never be none here"));
    }

    let skip_write = self.dev_context.options.skip_write;

    let scan_mode = if self.input.requires_full_rebuild() {
      ScanMode::Full
    } else {
      ScanMode::Partial(
        self.input.changed_files().iter().map(|p| p.to_string_lossy().into()).collect(),
      )
    };
    let scan_output = bundler.scan(scan_mode).await;
    let build_result = match scan_output {
      Ok(scan_output) => {
        if skip_write {
          bundler.bundle_generate(scan_output).await
        } else {
          bundler.bundle_write(scan_output).await
        }
      }
      Err(scan_error) => Err(scan_error),
    };

    if build_result.is_err() {
      tracing::error!("Build failed for changed files: {:#?}", self.input.changed_files());
    }

    // Call on_output callback if provided
    if let Some(on_output) = self.dev_context.options.on_output.as_ref() {
      on_output(build_result);
    }

    tracing::trace!(
      "`BuildStatus` finished building with changed files: {:#?}",
      self.input.changed_files()
    );
    self.bundler_cache = Some(bundler.take_cache());
    Ok(())
  }
}
