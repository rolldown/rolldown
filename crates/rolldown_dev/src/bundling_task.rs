use std::sync::{Arc, atomic::Ordering};

use arcstr::ArcStr;
use async_lock::Mutex;
use rolldown_common::{HmrUpdate, ScanMode, WatcherChangeKind};
use rolldown_utils::{dashmap::FxDashSet, indexmap::FxIndexMap};

use rolldown::Bundler;

use crate::{
  BundleOutput,
  dev_context::SharedDevContext,
  types::{coordinator_msg::CoordinatorMsg, error_stage::ErrorStage, task_input::TaskInput},
};

pub struct BundlingTask {
  pub input: TaskInput,
  pub bundler: Arc<Mutex<Bundler>>,
  pub dev_context: SharedDevContext,
  /// Set when `watch_change` hook or `generate_hmr_updates` errored.
  hmr_errored: bool,
  /// Set when `rebuild()` errored. Takes precedence over `hmr_errored`
  /// when deriving the final stage — see `final_error_stage`.
  rebuild_errored: bool,
  has_rebuild_happen: bool,
  /// The watch list the HMR stage added its loaded files to, kept when the rebuild that
  /// followed replaced it. See `rebuild`.
  hmr_stage_watch_files: Option<Arc<FxDashSet<ArcStr>>>,
}

impl BundlingTask {
  pub fn new(
    input: TaskInput,
    bundler: Arc<Mutex<Bundler>>,
    dev_context: SharedDevContext,
  ) -> Self {
    Self {
      input,
      bundler,
      dev_context,
      has_rebuild_happen: false,
      hmr_errored: false,
      rebuild_errored: false,
      hmr_stage_watch_files: None,
    }
  }

  /// Rebuild precedes Hmr: if both stages errored in the same task (an
  /// `HmrRebuild` whose HMR error went to a callback and whose rebuild then
  /// failed too), `Rebuild` is reported so recovery forces a fresh rebuild on
  /// the next file change.
  fn final_error_stage(&self) -> Option<ErrorStage> {
    if self.rebuild_errored {
      Some(ErrorStage::Rebuild)
    } else if self.hmr_errored {
      Some(ErrorStage::Hmr)
    } else {
      None
    }
  }

  /// The resolver caches misses as well as hits, and `clear_resolver_cache`
  /// is its only invalidation. Clear it when this task could resolve a path
  /// differently than the last one did. See
  /// `internal-docs/dev-engine/implementation.md` §9c.
  fn should_clear_resolver_cache(&self) -> bool {
    // A failed task may have cached the miss that made it fail. The file
    // can be back without a create event: a missing file the watcher never
    // saw, followed by an importer edit, or a recreate reported as an update.
    self.dev_context.last_task_errored.load(Ordering::Relaxed)
      || self.input.changed_files().iter().any(|(path, event)| {
        matches!(event, WatcherChangeKind::Create | WatcherChangeKind::Delete)
          || path.file_name().is_some_and(|name| name == "package.json")
      })
  }

  pub async fn run(mut self) {
    tracing::trace!("[BundlingTask] starts to run.\n - Task Input: {:#?}", self.input);
    self.run_inner().await;

    let has_generated_bundle_output = self.has_rebuild_happen;
    let error_stage = self.final_error_stage();
    // Feeds the next task's noop-upgrade check and resolver cache clear — see
    // `DevContext::last_task_errored`.
    self.dev_context.last_task_errored.store(error_stage.is_some(), Ordering::Relaxed);

    tracing::trace!(
      "[BundlingTask] completed\n - has_generated_bundle_output: {has_generated_bundle_output:?}",
    );

    self.dev_context.coordinator_tx.unbounded_send(CoordinatorMsg::BundleCompleted {
      error_stage,
      has_generated_bundle_output,
      hmr_stage_watch_files: self.hmr_stage_watch_files,
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

    // A tsconfig edit affects every module the tsconfig governs, which HMR
    // patches and partial scans cannot represent. Clear the caches, tell
    // clients to fully reload, and fall back to a full rebuild. A lost HMR
    // update (`Bundler::has_lost_hmr_update`) takes the same path in every
    // task kind: no client ran its edit. This includes the `FullBuild` that
    // recovers from a failed full build; nothing else reloads the clients then.
    let reload_reason = {
      let bundler = self.bundler.lock().await;
      let changed_tsconfig = self
        .input
        .changed_files()
        .keys()
        .any(|path| bundler.options().transform_options.is_known_tsconfig(path));
      // A bare full build carries no changed-file list (startup, restart,
      // failure recovery), so whether a tsconfig changed cannot be answered
      // there. Clear defensively; full builds are rare and the clears are
      // cheap.
      if changed_tsconfig || self.input.requires_full_rebuild() {
        bundler.clear_resolver_cache();
        bundler.clear_transform_tsconfig_cache();
      } else if self.should_clear_resolver_cache() {
        bundler.clear_resolver_cache();
      }
      let reason = if changed_tsconfig {
        tracing::trace!("[BundlingTask] detects a tsconfig change, upgrading to a full rebuild");
        Some("tsconfig change")
      } else if bundler.has_lost_hmr_update() {
        tracing::trace!(
          "[BundlingTask] an earlier HMR update was lost, upgrading to a full rebuild"
        );
        Some("an earlier hot update failed")
      } else {
        None
      };
      if reason.is_some() {
        // Before the reload goes out, so no client gets a patch for the output it leaves.
        bundler.begin_hmr_full_reload();
      }
      reason
    };
    if let Some(reload_reason) = reload_reason {
      if let Some(on_hmr_updates) = self.dev_context.options.on_hmr_updates.as_ref() {
        let mut changed_files = self
          .input
          .changed_files()
          .keys()
          .map(|path| path.to_string_lossy().to_string())
          .collect::<Vec<_>>();
        // A `FullBuild` carries no changed files, and Vite ignores an update with an empty
        // `changedFiles`, so the reload would be dropped. `"*"` is not a path, only a
        // workaround: remove it once Vite delivers a `FullReload` with no changed files.
        if changed_files.is_empty() {
          changed_files.push("*".to_owned());
        }
        on_hmr_updates(Ok((
          HmrUpdate::FullReload { reason: reload_reason.to_owned() },
          changed_files,
        )));
      }
      self.input = TaskInput::FullBuild;
    }

    if self.input.require_generate_hmr_update() {
      tracing::trace!("[BundlingTask] starts to generate HMR updates");
      let may_continue = self.generate_hmr_updates().await;
      tracing::trace!("[BundlingTask] completed generating HMR updates");
      // Stop only when HMR errored AND no callback was registered to receive
      // the error — preserving the pre-refactor `?` short-circuit. When the
      // consumer was informed via callback, the rebuild still runs.
      if !may_continue {
        return;
      }
    }

    // No eager rebuild for reload-bound updates: the server no longer decides reloads.
    // A patch-only task leaves `has_stale_bundle_output` set, so a client that reloads
    // itself lands on the stale-access regeneration path (fallback page → rebuild →
    // reload onto fresh output).

    if self.input.requires_rebuild() {
      self.has_rebuild_happen = true;
      self.rebuild().await;
    }
  }

  /// Returns `true` if subsequent build stages may continue.
  /// Callers should skip subsequent build stages on `false`.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn generate_hmr_updates(&mut self) -> bool {
    let mut bundler = self.bundler.lock().await;
    let changed_files = self
      .input
      .changed_files()
      .iter()
      .map(|(p, event)| (p.to_string_lossy().to_string(), *event))
      .collect::<FxIndexMap<_, _>>();

    // After an errored task the unchanged-output suppression is disabled, so a byte-identical
    // recovery still reaches clients stuck on that error (a `Noop` would send nothing).
    let hmr_result = bundler
      .compute_hmr_update_for_file_changes(
        &changed_files,
        self.dev_context.last_task_errored.load(Ordering::Relaxed),
        self.dev_context.options.hot_update,
      )
      .await;

    let succeeded = hmr_result.is_ok();
    let has_callback = self.dev_context.options.on_hmr_updates.is_some();
    if let Err(err) = &hmr_result {
      tracing::error!("[BundlingTask] failed to generate HMR updates: {:?}", err);
      self.hmr_errored = true;
    }

    // Deliver any assets emitted while computing this update (e.g. an image newly
    // imported by the changed module) BEFORE sending the update, so the consumer can
    // register/serve them before the client requests them. A pure HMR update never
    // triggers `on_output`, so this is their only delivery path.
    if succeeded {
      if let Some(on_additional_assets) = self.dev_context.options.on_additional_assets.as_ref() {
        let mut output = BundleOutput::default();
        bundler.file_emitter.add_additional_files(&mut output.assets, &mut output.warnings);
        if !output.assets.is_empty() {
          on_additional_assets(output);
        }
      }
    }

    // Call on_hmr_updates callback if provided
    if let Some(on_hmr_updates) = self.dev_context.options.on_hmr_updates.as_ref() {
      on_hmr_updates(
        hmr_result.map(|update| (update, changed_files.iter().map(|(p, _)| p.clone()).collect())),
      );
    }

    // Continue when HMR succeeded, or when the consumer was informed of the
    // failure via the callback. Stop only on an undeliverable error.
    succeeded || has_callback
  }

  #[tracing::instrument(level = "trace", skip_all)]
  async fn rebuild(&mut self) {
    let mut bundler = self.bundler.lock().await;

    // The HMR stage of this task added the files it loaded to the latest build's watch list.
    // The build below starts a new, empty list and does not load those cached modules again,
    // so keep the HMR stage's list for the coordinator to register.
    // See internal-docs/dev-engine/implementation.md §10.
    let hmr_stage_watch_files =
      self.input.require_generate_hmr_update().then(|| Arc::clone(bundler.watch_files()));

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
    self.hmr_stage_watch_files =
      hmr_stage_watch_files.filter(|list| !Arc::ptr_eq(list, bundler.watch_files()));

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
