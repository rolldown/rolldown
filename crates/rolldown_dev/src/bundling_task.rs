use std::{
  ops::{Deref, DerefMut},
  sync::{Arc, atomic::AtomicU32},
};

use arcstr::ArcStr;
use async_lock::Mutex;
use rolldown_common::{ClientHmrInput, ClientHmrUpdate, HmrUpdate, ScanMode};
use rolldown_dev_common::types::DevCallbackResult;
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxHashMap;

use rolldown::Bundler;

use crate::{
  BundleOutput,
  dev_context::SharedDevContext,
  types::{
    coordinator_msg::CoordinatorMsg, error_stage::ErrorStage, pending_payload::PendingPayload,
    task_input::TaskInput,
  },
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

  pub async fn run(mut self) -> DevCallbackResult {
    tracing::trace!("[BundlingTask] starts to run.\n - Task Input: {:#?}", self.input);
    let callback_result = self.run_inner().await;

    let has_generated_bundle_output = self.has_rebuild_happen;
    let error_stage = self.final_error_stage();
    let callback_error = callback_result.as_ref().err().cloned();

    tracing::trace!(
      "[BundlingTask] completed\n - has_generated_bundle_output: {has_generated_bundle_output:?}",
    );

    self.dev_context.coordinator_tx.unbounded_send(CoordinatorMsg::BundleCompleted {
      error_stage,
      has_generated_bundle_output,
      callback_error,
    }).expect(
      "Coordinator channel closed while sending BundleCompleted - coordinator terminated unexpectedly"
    );

    callback_result
  }

  async fn run_inner(&mut self) -> DevCallbackResult {
    let watch_change_error = {
      let bundler = self.bundler.lock().await;
      let mut watch_change_error = None;
      for (changed_file, event) in self.input.changed_files() {
        if let Some(plugin_driver) =
          bundler.last_bundle_handle.as_ref().map(rolldown::BundleHandle::plugin_driver)
        {
          if let Err(err) = plugin_driver.watch_change(changed_file.to_str().unwrap(), *event).await
          {
            tracing::error!("[BundlingTask] watchChange hook failed: {err:?}");
            watch_change_error = Some(err.into());
            break;
          }
        }
      }
      watch_change_error
    };
    if let Some(error) = watch_change_error {
      // Classified as Hmr stage: the next Hmr task re-runs watch_change,
      // which is sufficient to retry the hook.
      self.hmr_errored = true;
      if let Some(on_output) = self.dev_context.options.on_output.as_ref() {
        on_output(Err(error)).await?;
      }
      return Ok(());
    }

    // A tsconfig edit affects every module the tsconfig governs, which HMR
    // patches and partial scans cannot represent. Clear the caches, tell
    // clients to fully reload, and fall back to a full rebuild.
    let changed_tsconfig = {
      let bundler = self.bundler.lock().await;
      let changed_tsconfig = self
        .input
        .changed_files()
        .keys()
        .any(|path| bundler.options().transform_options.is_known_tsconfig(path));
      if changed_tsconfig {
        tracing::trace!("[BundlingTask] detects a tsconfig change, upgrading to a full rebuild");
      }
      // A bare full build carries no changed-file list (startup, restart,
      // failure recovery), so whether a tsconfig changed cannot be answered
      // there. Clear defensively; full builds are rare and the clears are
      // cheap.
      if changed_tsconfig || self.input.requires_full_rebuild() {
        bundler.clear_resolver_cache();
        bundler.clear_transform_tsconfig_cache();
      }
      changed_tsconfig
    };
    if changed_tsconfig {
      if let Some(on_hmr_updates) = self.dev_context.options.on_hmr_updates.as_ref() {
        let changed_files = self
          .input
          .changed_files()
          .keys()
          .map(|path| path.to_string_lossy().to_string())
          .collect::<Vec<_>>();
        let updates = (self.dev_context.clients.lock().await)
          .keys()
          .map(|client_id| ClientHmrUpdate {
            client_id: client_id.clone(),
            update: HmrUpdate::FullReload { reason: "tsconfig change".to_owned() },
          })
          .collect();
        on_hmr_updates(Ok((updates, changed_files))).await?;
      }
      self.input = TaskInput::FullBuild;
    }

    if self.input.require_generate_hmr_update() {
      tracing::trace!("[BundlingTask] starts to generate HMR updates");
      let may_continue = self.generate_hmr_updates().await?;
      tracing::trace!("[BundlingTask] completed generating HMR updates");
      // Stop only when HMR errored AND no callback was registered to receive
      // the error — preserving the pre-refactor `?` short-circuit. When the
      // consumer was informed via callback, the rebuild still runs.
      if !may_continue {
        return Ok(());
      }
    }

    // No eager rebuild for reload-bound updates: the server no longer decides reloads.
    // A patch-only task leaves `has_stale_bundle_output` set, so a client that reloads
    // itself lands on the stale-access regeneration path (fallback page → rebuild →
    // reload onto fresh output).

    if self.input.requires_rebuild() {
      self.has_rebuild_happen = true;
      self.rebuild().await?;
    }

    Ok(())
  }

  /// Returns `true` if subsequent build stages may continue.
  /// Callers should skip subsequent build stages on `false`.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn generate_hmr_updates(&mut self) -> DevCallbackResult<bool> {
    let mut bundler = self.bundler.lock().await;
    let changed_files = self
      .input
      .changed_files()
      .iter()
      .map(|(p, event)| (p.to_string_lossy().to_string(), *event))
      .collect::<FxIndexMap<_, _>>();

    // Read-only per-client inputs for this push. No seq here: it is assigned after compute,
    // only to the patches we actually deliver (see below). Snapshot the ids and ship maps
    // and release the clients lock before the compute await — the compute only reads the
    // snapshot, and a delivery notification landing mid-compute is folded into the next
    // push either way; holding the lock would block connect/disconnect and delivery
    // notifications for the whole rebuild.
    let client_snapshots: Vec<(String, FxHashMap<ArcStr, u32>)> = {
      let client_sessions = self.dev_context.clients.lock().await;
      client_sessions
        .iter()
        .map(|(client_key, client)| (client_key.clone(), client.shipped.clone()))
        .collect()
    };
    let client_inputs: Vec<ClientHmrInput> = client_snapshots
      .iter()
      .map(|(client_id, shipped)| ClientHmrInput { client_id: client_id.as_str(), shipped })
      .collect();

    // Compute HMR updates for all clients in one call
    let mut stamp_table = self.dev_context.stamp_table.lock().await;
    let mut hmr_result = bundler
      .compute_hmr_update_for_file_changes(
        &changed_files,
        &client_inputs,
        &mut stamp_table,
        Arc::clone(&self.next_hmr_patch_id),
      )
      .await;
    drop(stamp_table);
    drop(client_inputs);

    // `seq` is incremented only when the client actually receives an update — i.e. an
    // `HmrUpdate::Patch`. A `HmrUpdate::Noop` sends nothing, so it never advances the
    // counter. The client enforces a strict `seq === lastSeq + 1`, so consuming a seq
    // without delivering an envelope would leave a gap and trigger a spurious full reload.
    // A client that disconnected during compute is simply absent here; its update is
    // dropped unstamped.
    if let Ok(client_updates) = &mut hmr_result {
      let mut client_sessions = self.dev_context.clients.lock().await;
      for update in client_updates.iter_mut() {
        if let HmrUpdate::Patch(patch) = &mut update.update {
          if let Some(session) = client_sessions.get_mut(&update.client_id) {
            session.next_seq += 1;
            patch.seq = session.next_seq;
          }
        }
      }
    }

    // Record each rendered patch as pending (only if successful): the delivery
    // notification max-merges its stamps into `shipped[C]` when the serving
    // middleware sees the response for `patch.filename` complete. `carried` is
    // handed over instead of cloned — the binding layer drops it (it stays
    // server-side), so the pending entry is its only consumer from here on.
    if let Ok(client_updates) = &mut hmr_result {
      for update in client_updates.iter_mut() {
        if let HmrUpdate::Patch(patch) = &mut update.update {
          self
            .dev_context
            .insert_pending_payload(
              patch.filename.clone(),
              PendingPayload {
                client_id: update.client_id.clone(),
                modules: std::mem::take(&mut patch.carried),
              },
            )
            .await;
        }
      }
    }

    let succeeded = hmr_result.is_ok();
    let has_callback = self.dev_context.options.on_hmr_updates.is_some();
    if let Err(err) = &hmr_result {
      tracing::error!("[BundlingTask] failed to generate HMR updates: {:?}", err);
      self.hmr_errored = true;
    }

    // Deliver any assets emitted while generating this HMR patch (e.g. an image
    // newly imported by the changed module) BEFORE sending the patch, so the
    // consumer can register/serve them before the client requests them. A pure
    // HMR patch never triggers `on_output`, so this is their only delivery path.
    let additional_assets = if succeeded {
      if let Some(on_additional_assets) = self.dev_context.options.on_additional_assets.as_ref() {
        let mut output = BundleOutput::default();
        bundler
          .file_emitter()
          .expect("a successful HMR update requires an installed bundle handle")
          .add_additional_files(&mut output.assets, &mut output.warnings);
        if output.assets.is_empty() {
          None
        } else {
          Some((Arc::clone(on_additional_assets), output))
        }
      } else {
        None
      }
    } else {
      None
    };
    drop(bundler);

    if let Some((on_additional_assets, output)) = additional_assets
      && let Err(error) = on_additional_assets(output).await
    {
      self.hmr_errored = true;
      return Err(error);
    }

    // Call on_hmr_updates callback if provided
    if let Some(on_hmr_updates) = self.dev_context.options.on_hmr_updates.as_ref() {
      if let Err(error) = on_hmr_updates(hmr_result.map(|client_updates| {
        (client_updates, changed_files.iter().map(|(p, _)| p.clone()).collect())
      }))
      .await
      {
        self.hmr_errored = true;
        return Err(error);
      }
    }

    // Continue when HMR succeeded, or when the consumer was informed of the
    // failure via the callback. Stop only on an undeliverable error.
    Ok(succeeded || has_callback)
  }

  #[tracing::instrument(level = "trace", skip_all)]
  async fn rebuild(&mut self) -> DevCallbackResult {
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
    } else {
      // The output just written is what any newly loading client evaluates; refresh
      // the top-level-evaluated snapshot new sessions freeze in at hello (`register_client`).
      let top_level_evaluated = {
        let stamp_table = self.dev_context.stamp_table.lock().await;
        bundler.compute_top_level_evaluated_modules(&stamp_table)
      };
      *self.dev_context.top_level_evaluated.lock().await = Arc::new(top_level_evaluated);
    }
    drop(bundler);

    // Call on_output callback if provided
    if let Some(on_output) = self.dev_context.options.on_output.as_ref() {
      if let Err(error) = on_output(build_result).await {
        self.rebuild_errored = true;
        return Err(error);
      }
    }

    Ok(())
  }
}
