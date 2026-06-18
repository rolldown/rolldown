use std::{
  ops::{Deref, DerefMut},
  sync::{Arc, atomic::AtomicU32},
};

use rolldown_common::{ClientHmrInput, ScanMode};
use rolldown_utils::indexmap::FxIndexMap;
use tokio::sync::Mutex;

use rolldown::Bundler;

use crate::{
  dev_context::SharedDevContext,
  types::{coordinator_msg::CoordinatorMsg, error_stage::ErrorStage, task_input::TaskInput},
};

pub struct BundlingTask {
  pub input: TaskInput,
  pub bundler: Arc<Mutex<Bundler>>,
  pub dev_context: SharedDevContext,
  pub next_hmr_patch_id: Arc<AtomicU32>,
  /// Set when `watch_change` hook or `generate_hmr_updates` errored.
  hmr_errored: bool,
  /// Set when `rebuild()` errored. Takes precedence over `hmr_errored`
  /// when deriving the final stage — see `final_error_stage`.
  rebuild_errored: bool,
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
      hmr_errored: false,
      rebuild_errored: false,
    }
  }

  /// Rebuild precedes Hmr: if both stages errored in the same task (only
  /// possible after the auto-upgrade rewrite), `Rebuild` is reported so
  /// recovery forces a fresh rebuild on the next file change.
  fn final_error_stage(&self) -> Option<ErrorStage> {
    if self.rebuild_errored {
      Some(ErrorStage::Rebuild)
    } else if self.hmr_errored {
      Some(ErrorStage::Hmr)
    } else {
      None
    }
  }

  pub async fn run(mut self) {
    tracing::trace!("[BundlingTask] starts to run.\n - Task Input: {:#?}", self.input);
    self.run_inner().await;

    let has_generated_bundle_output = self.has_rebuild_happen;
    let error_stage = self.final_error_stage();

    tracing::trace!(
      "[BundlingTask] completed\n - has_generated_bundle_output: {has_generated_bundle_output:?}",
    );

    self.dev_context.coordinator_tx.send(CoordinatorMsg::BundleCompleted {
      error_stage,
      has_generated_bundle_output,
    }).expect(
      "Coordinator channel closed while sending BundleCompleted - coordinator terminated unexpectedly"
    );
  }

  async fn run_inner(&mut self) {
    {
      let bundler = self.bundler.lock().await;
      for (changed_file, event) in self.input.changed_files() {
        if let Some(plugin_driver) =
          bundler.last_bundle_handle.as_ref().map(rolldown::BundleHandle::plugin_driver)
        {
          if let Err(err) = plugin_driver.watch_change(changed_file.to_str().unwrap(), *event).await
          {
            tracing::error!("[BundlingTask] watchChange hook failed: {err:?}");
            // Classified as Hmr stage: the next Hmr task re-runs watch_change,
            // which is sufficient to retry the hook.
            self.hmr_errored = true;
            if let Some(on_output) = self.dev_context.options.on_output.as_ref() {
              on_output(Err(err.into()));
            }
            return;
          }
        }
      }
    }

    let mut has_full_reload_update = false;
    if self.input.require_generate_hmr_update() {
      tracing::trace!("[BundlingTask] starts to generate HMR updates");
      let may_continue = self.generate_hmr_updates(&mut has_full_reload_update).await;
      tracing::trace!(
        "[BundlingTask] completed generating HMR updates\n - has_full_reload_update: {has_full_reload_update}"
      );
      // Stop only when HMR errored AND no callback was registered to receive
      // the error — preserving the pre-refactor `?` short-circuit. When the
      // consumer was informed via callback, the rebuild still runs.
      if !may_continue {
        return;
      }
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
      self.rebuild().await;
    }
  }

  /// Returns `true` if subsequent build stages may continue.
  /// Callers should skip subsequent build stages on `false`.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn generate_hmr_updates(&mut self, has_full_reload_update: &mut bool) -> bool {
    let mut bundler = self.bundler.lock().await;
    let changed_files = self
      .input
      .changed_files()
      .iter()
      .map(|(p, event)| (p.to_string_lossy().to_string(), *event))
      .collect::<FxIndexMap<_, _>>();

    let client_sessions = self.dev_context.clients.lock().await;
    let client_inputs: Vec<ClientHmrInput> = client_sessions
      .iter()
      .map(|(client_key, client)| ClientHmrInput {
        client_id: client_key,
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

    let succeeded = hmr_result.is_ok();
    let has_callback = self.dev_context.options.on_hmr_updates.is_some();
    if let Err(err) = &hmr_result {
      tracing::error!("[BundlingTask] failed to generate HMR updates: {:?}", err);
      self.hmr_errored = true;
    }

    // Call on_hmr_updates callback if provided
    if let Some(on_hmr_updates) = self.dev_context.options.on_hmr_updates.as_ref() {
      on_hmr_updates(hmr_result.map(|client_updates| {
        (client_updates, changed_files.iter().map(|(p, _)| p.clone()).collect())
      }));
    }

    // Continue when HMR succeeded, or when the consumer was informed of the
    // failure via the callback. Stop only on an undeliverable error.
    succeeded || has_callback
  }

  #[tracing::instrument(level = "trace", skip_all)]
  async fn rebuild(&mut self) {
    let mut bundler = self.bundler.lock().await;

    // TODO: hyf0 `skip_write` in watch mode won't trigger generate stage, need to investigate why.
    let skip_write = self.dev_context.options.skip_write;

    let scan_mode = if self.input.requires_full_rebuild() {
      ScanMode::Full
    } else {
      ScanMode::Partial(
        self.input.changed_files().iter().map(|(p, _)| p.to_string_lossy().into()).collect(),
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
      self.rebuild_errored = true;
    }

    // Call on_output callback if provided
    if let Some(on_output) = self.dev_context.options.on_output.as_ref() {
      on_output(build_result);
    }
  }
}
