use std::{
  ops::{Deref, DerefMut},
  sync::{Arc, atomic::AtomicU32},
};

use rolldown_common::{ClientHmrInput, ScanMode, WatcherChangeKind};
use rolldown_error::BuildResult;
use tokio::sync::Mutex;

use rolldown::Bundler;

use crate::{
  dev_context::SharedDevContext,
  types::{coordinator_msg::CoordinatorMsg, task_input::TaskInput},
};

pub struct BundlingTask {
  pub input: TaskInput,
  pub bundler: Arc<Mutex<Bundler>>,
  pub dev_context: SharedDevContext,
  pub next_hmr_patch_id: Arc<AtomicU32>,
  has_encountered_error: bool,
  has_rebuild_happen: bool,
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
  pub fn new(
    input: TaskInput,
    bundler: Arc<Mutex<Bundler>>,
    dev_context: SharedDevContext,
    next_hmr_patch_id: Arc<AtomicU32>,
  ) -> Self {
    Self {
      input,
      bundler,
      dev_context,
      next_hmr_patch_id,
      has_rebuild_happen: false,
      has_encountered_error: false,
    }
  }

  pub async fn run(mut self) {
    tracing::trace!("[BundlingTask] starts to run.\n - Task Input: {:#?}", self.input);
    let task_run_result = self.run_inner().await;

    if let Err(err) = &task_run_result {
      tracing::error!("[BundlingTask] fails to run");
      // FIXME: Should handle the error properly.
      eprintln!("Bundling task run with error: {err}"); // FIXME: handle this error
    }

    let has_generated_bundle_output = self.has_rebuild_happen;
    let has_encountered_error = self.has_encountered_error || task_run_result.is_err();

    tracing::trace!(
      "[BundlingTask] completed\n - has_generated_bundle_output: {has_generated_bundle_output:?}",
    );

    self.dev_context.coordinator_tx.send(CoordinatorMsg::BundleCompleted {
      has_encountered_error,
      has_generated_bundle_output,
    }).expect(
      "Coordinator channel closed while sending BundleCompleted - coordinator terminated unexpectedly"
    );
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    {
      let bundler = self.bundler.lock().await;
      for changed_file in self.input.changed_files() {
        if let Some(plugin_driver) =
          bundler.last_bundle_handle.as_ref().map(rolldown::BundleHandle::plugin_driver)
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
      tracing::trace!("[BundlingTask] starts to generate HMR updates");
      self.generate_hmr_updates(&mut has_full_reload_update).await?;
      tracing::trace!(
        "[BundlingTask] completed generating HMR updates\n - has_full_reload_update: {has_full_reload_update}"
      );
    }

    // If the rebuild strategy is auto and there's a full reload update, we need to rebuild.
    // Convert Hmr to HmrRebuild if needed
    if self.dev_context.options.rebuild_strategy.is_auto()
      && has_full_reload_update
      && !self.input.requires_rebuild()
    {
      tracing::trace!("[BundlingTask] detects full reload HMR update, upgrading to HmrRebuild");
      if let Some(changed_files) = self.input.changed_files_mut() {
        self.input = TaskInput::HmrRebuild { changed_files: std::mem::take(changed_files) };
      }
    }

    if self.input.requires_rebuild() {
      self.has_rebuild_happen = true;
      self.rebuild().await?;
    }

    Ok(())
  }

  pub async fn generate_hmr_updates(
    &mut self,
    has_full_reload_update: &mut bool,
  ) -> BuildResult<()> {
    // Yield to the tokio scheduler before acquiring the bundler lock.
    // This gives the Node.js event loop a chance to process pending timers (e.g., setTimeout)
    // when HMR tasks run in rapid succession. Without this, the UV loop can get starved
    // and never reach the timer phase, causing setTimeout callbacks in plugins to never fire.
    tokio::task::yield_now().await;

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

    if let Err(err) = &hmr_result {
      tracing::error!("[BundlingTask] failed to generate HMR updates: {:?}", err);
      self.has_encountered_error = true;
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

  async fn rebuild(&mut self) -> BuildResult<()> {
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

    tracing::trace!(
      "[BundlingTask] starts to perform rebuild\n - skip_write: {skip_write:?}\n - scan_mode: {scan_mode:?}"
    );
    let build_result = if skip_write {
      bundler.incremental_generate(scan_mode).await
    } else {
      bundler.incremental_write(scan_mode).await
    };

    if let Err(err) = &build_result {
      tracing::error!("[BundlingTask] rebuild failed: {:?}", err);
      self.has_encountered_error = true;
    }

    // Call on_output callback if provided
    if let Some(on_output) = self.dev_context.options.on_output.as_ref() {
      on_output(build_result);
    }

    Ok(())
  }
}
