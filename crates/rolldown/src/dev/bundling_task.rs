use std::{
  ops::{Deref, DerefMut},
  sync::{Arc, atomic::AtomicU32},
};

use rolldown_common::{ClientHmrInput, ScanMode, WatcherChangeKind};
use rolldown_error::BuildResult;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  dev::{
    dev_context::SharedDevContext,
    types::{coordinator_msg::CoordinatorMsg, task_input::TaskInput},
  },
};

pub struct BundlingTask {
  pub input: TaskInput,
  pub bundler: Arc<Mutex<Bundler>>,
  pub dev_context: SharedDevContext,
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
    let task_run_result = self.run_inner().await;

    if let Err(err) = &task_run_result {
      // FIXME: Should handle the error properly.
      eprintln!("Bundling task run with error: {err}"); // FIXME: handle this error
    }

    // Check final task type after run_inner (task may have been converted)
    let task_required_rebuild = self.input.requires_rebuild();

    self.dev_context.coordinator_tx.send(CoordinatorMsg::BuildCompleted {
      result: task_run_result,
      task_required_rebuild,
    }).expect(
      "Coordinator channel closed while sending BuildCompleted - coordinator terminated unexpectedly"
    );
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    {
      let bundler = self.bundler.lock().await;
      for changed_file in self.input.changed_files() {
        if let Some(plugin_driver) =
          bundler.last_bundle_handle.as_ref().map(|ctx| &ctx.plugin_driver)
        {
          plugin_driver
            // FIXME: use proper WatcherChangeKind for created/removed files.
            .watch_change(changed_file.to_str().unwrap(), WatcherChangeKind::Update)
            .await?;
        }
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

  pub async fn generate_hmr_updates(
    &mut self,
    has_full_reload_update: &mut bool,
  ) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;
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

  async fn rebuild(&self) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;

    // TODO: hyf0 `skip_write` in watch mode won't trigger generate stage, need to investigate why.
    let skip_write = self.dev_context.options.skip_write;

    let scan_mode = if self.input.requires_full_rebuild() {
      ScanMode::Full
    } else {
      ScanMode::Partial(
        self.input.changed_files().iter().map(|p| p.to_string_lossy().into()).collect(),
      )
    };

    let build_result = if skip_write {
      bundler.incremental_generate(scan_mode).await
    } else {
      bundler.incremental_write(scan_mode).await
    };

    let ret = if build_result.is_err() {
      tracing::error!("Build failed for changed files: {:#?}", self.input.changed_files());
      Err(anyhow::format_err!("Err"))
    } else {
      tracing::info!("Build succeeded for changed files: {:#?}", self.input.changed_files());
      Ok(())
    };

    // Call on_output callback if provided
    if let Some(on_output) = self.dev_context.options.on_output.as_ref() {
      on_output(build_result);
    }

    tracing::trace!(
      "`BuildStatus` finished building with changed files: {:#?}",
      self.input.changed_files()
    );

    ret?;
    Ok(())
  }
}
