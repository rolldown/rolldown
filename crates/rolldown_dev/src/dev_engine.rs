use std::sync::{
  Arc,
  atomic::{AtomicBool, AtomicU32},
};

use anyhow::Context;
use arcstr::ArcStr;
use async_lock::Mutex;
use futures::channel::mpsc::unbounded;
use futures::{FutureExt, future::Shared};
#[cfg(feature = "testing")]
use rolldown_common::WatcherChangeKind;
use rolldown_common::{HmrLazyChunkOutput, HmrStampTable};
use rolldown_error::{BuildResult, ResultExt};
use rolldown_fs_watcher::{FsWatcher, FsWatcherConfig};
use rolldown_utils::{dashmap::FxDashSet, futures::try_spawn};
use rustc_hash::FxHashMap;
#[cfg(feature = "testing")]
use rustc_hash::FxHashSet;

use rolldown::{Bundler, BundlerBuilder, BundlerConfig, NormalizedBundlerOptions};

use crate::{
  BundleOutput, DevOptions, SharedClients,
  bundle_coordinator::BundleCoordinator,
  dev_context::{DevContext, PinBoxSendStaticFuture},
  normalize_dev_options,
  type_aliases::CoordinatorSender,
  types::{
    coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot,
    error_stage::ErrorStage, pending_payload::PendingPayload,
  },
};

use crate::ClientSession;
#[cfg(feature = "testing")]
use rolldown_utils::indexmap::FxIndexMap;
#[cfg(feature = "testing")]
use std::path::PathBuf;

type CoordinatorTaskResult = Result<(), Arc<str>>;

pub struct CoordinatorState {
  coordinator: Option<BundleCoordinator>,
  handle: Option<Shared<PinBoxSendStaticFuture<CoordinatorTaskResult>>>,
}

pub struct DevEngine {
  coordinator_sender: CoordinatorSender,
  bundler: Arc<Mutex<Bundler>>,
  /// Shared dev context, kept so out-of-coordinator entry points (e.g.
  /// `compile_lazy_entry`) can reach `options.on_additional_assets`.
  dev_context: Arc<DevContext>,
  coordinator_state: Mutex<CoordinatorState>,
  pub clients: SharedClients,
  is_closed: AtomicBool,
  /// The engine's single patch-id counter, shared with the coordinator's bundling
  /// tasks. Both counters' consumers format filenames as `hmr_patch_{id}.js` /
  /// `lazy_compile_{id}.js`, and pending-payload entries are keyed by those
  /// filenames — so two independent counters would let two different payloads
  /// collide on one key.
  next_hmr_patch_id: Arc<AtomicU32>,
  /// Live handle to the plugin-facing module infos — the same map plugin contexts read.
  /// Full builds clear it in place (the `Arc` identity is stable), so lock-free reads
  /// here always observe the latest build. Powers the engine-level module queries.
  module_infos: rolldown_common::SharedModuleInfoDashMap,
  module_side_effects_overrides: rolldown_common::SharedModuleSideEffectsOverrides,
}

impl DevEngine {
  pub fn new(mut config: BundlerConfig, options: DevOptions) -> BuildResult<Self> {
    // The HMR stage diffs against the scan-stage snapshot, which is only kept
    // when incremental build is on. Without it the first file change unwraps a
    // `None` snapshot and panics the worker. `prepare_build_context` already
    // forces this for `dev_mode`, but `dev()` is reachable without it.
    config.options.experimental.get_or_insert_default().incremental_build = Some(true);

    // Build the bundler from config
    let bundler = BundlerBuilder::default()
      .with_options(config.options)
      .with_plugins(config.plugins)
      .build()?;

    let module_infos = bundler.module_infos();
    let module_side_effects_overrides = bundler.module_side_effects_overrides();
    let cwd = bundler.options().cwd.clone();
    let bundler = Arc::new(Mutex::new(bundler));

    let normalized_options = normalize_dev_options(options);

    let (coordinator_tx, coordinator_rx) = unbounded::<CoordinatorMsg>();

    let clients = SharedClients::default();

    // ONE patch-id counter for the whole engine (bundling tasks AND lazy
    // compiles) — see the field doc on `next_hmr_patch_id`.
    let next_hmr_patch_id = Arc::new(AtomicU32::new(0));

    let ctx = Arc::new(DevContext {
      options: normalized_options,
      coordinator_tx: coordinator_tx.clone(),
      clients: Arc::clone(&clients),
      stamp_table: Arc::new(Mutex::new(HmrStampTable::default())),
      pending_payloads: Arc::new(Mutex::new(FxHashMap::default())),
      top_level_evaluated: Mutex::new(Arc::new(FxHashMap::default())),
      last_task_errored: std::sync::atomic::AtomicBool::new(false),
    });

    let watcher_config = FsWatcherConfig {
      enabled: !ctx.options.disable_watcher,
      poll_interval: ctx.options.poll_interval,
      debounce_delay: ctx.options.debounce_delay,
      compare_contents_for_polling: ctx.options.compare_contents_for_polling,
      debounce_tick_rate: ctx.options.debounce_tick_rate,
      use_polling: ctx.options.use_polling,
      use_debounce: ctx.options.use_debounce,
      ignored: ctx.options.watch_exclude.clone(),
      cwd,
    };

    let event_handler = BundleCoordinator::create_watcher_event_handler(coordinator_tx.clone());
    let watcher = FsWatcher::new(event_handler, &watcher_config)?;

    let coordinator = BundleCoordinator::new(
      Arc::clone(&bundler),
      Arc::clone(&ctx),
      coordinator_rx,
      watcher,
      watcher_config.cwd,
      Arc::clone(&next_hmr_patch_id),
    );

    Ok(Self {
      coordinator_sender: coordinator_tx,
      bundler,
      dev_context: Arc::clone(&ctx),
      coordinator_state: Mutex::new(CoordinatorState {
        coordinator: Some(coordinator),
        handle: None,
      }),
      clients,
      is_closed: AtomicBool::new(false),
      next_hmr_patch_id,
      module_infos,
      module_side_effects_overrides,
    })
  }

  /// Same data the plugin-context `getModuleInfo` returns, readable from the engine
  /// handle at any time (no hook context needed).
  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<rolldown_common::ModuleInfo>> {
    let info = self.module_infos.get(module_id).map(|entry| Arc::clone(entry.value()))?;
    Some(self.module_side_effects_overrides.apply_to_info(info))
  }

  pub fn get_module_ids(&self) -> Vec<arcstr::ArcStr> {
    self.module_infos.iter().map(|entry| entry.key().clone()).collect()
  }

  pub async fn run(&self) -> BuildResult<()> {
    let mut coordinator_state = self.coordinator_state.lock().await;

    // Spawn the coordinator
    let Some(coordinator) = coordinator_state.coordinator.take() else {
      // The coordinator is already running.
      return Ok(());
    };
    let join_handle = try_spawn(coordinator.run()).map_err(|(error, _)| {
      anyhow::anyhow!("DevEngine coordinator task submission failed: {error}")
    })?;
    let coordinator_handle = Box::pin(async move {
      join_handle
        .await
        .map_err(|error| Arc::<str>::from(format!("DevEngine coordinator task failed: {error}")))
    }) as PinBoxSendStaticFuture<CoordinatorTaskResult>;
    coordinator_state.handle = Some(coordinator_handle.shared());
    drop(coordinator_state);

    // Wait for initial build to complete. It's ok if the initial build fails, we just let it pass.
    // Recovering from errors is handled by other parts of the system.
    self.ensure_latest_bundle_output().await?;

    Ok(())
  }

  /// TODO: do we really need this as a public API? What's the use case?
  pub async fn wait_for_close(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    let coordinator_state = self.coordinator_state.lock().await;
    if let Some(coordinator_handle) = coordinator_state.handle.clone() {
      if let Err(error) = coordinator_handle.await {
        return Err(anyhow::anyhow!("{error}").into());
      }
    }
    Ok(())
  }

  /// Wait for ongoing bundle to finish if there is one.
  ///
  /// If the `DevEngine` is closed while waiting, this method will return early without error.
  pub async fn wait_for_ongoing_bundle(&self) -> BuildResult<()> {
    if self.is_closed() {
      return Ok(());
    }

    let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
    if let Err(err) =
      self.coordinator_sender.unbounded_send(CoordinatorMsg::GetState { reply: reply_sender })
    {
      if self.is_closed() {
        return Ok(());
      }
      return (Err(err))
        .map_err_to_unhandleable()
        .context("DevEngine: failed to send GetState to coordinator")?;
    }

    let Ok(status) = reply_receiver.await else {
      if self.is_closed() {
        return Ok(());
      }
      return Err(anyhow::anyhow!("DevEngine: coordinator closed before responding to GetState"))?;
    };

    if let Some(bundling_future) = status.running_future {
      bundling_future.await;
    }

    Ok(())
  }

  pub async fn get_bundle_state(&self) -> BuildResult<BundleState> {
    self.create_error_if_closed()?;

    let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
    self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::GetState { reply: reply_sender })
      .map_err_to_unhandleable()
      .context(
        "DevEngine: failed to send GetState to coordinator within has_latest_bundle_output",
      )?;

    let status = reply_receiver.await.map_err_to_unhandleable().context(
      "DevEngine: coordinator closed before responding to GetStatus within get_bundle_state",
    )?;

    Ok(status.into())
  }

  /// Ensure there's latest bundle output available for browser loading/reloading scenarios.
  ///
  /// If the `DevEngine` is closed while waiting, this method will return early without error.
  pub async fn ensure_latest_bundle_output(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    let mut loop_count = 0u32;
    loop {
      loop_count += 1;
      if loop_count > 100 {
        debug_assert!(
          false,
          "[DevEngine] ensure_latest_bundle_output has looped {loop_count} times, something is definitely wrong",
        );
        tracing::warn!(
          "[DevEngine] ensure_latest_bundle_output has looped {loop_count} times, something might be wrong",
        );
        break;
      }
      let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
      if let Err(err) = self
        .coordinator_sender
        .unbounded_send(CoordinatorMsg::EnsureLatestBundleOutput { reply: reply_sender })
      {
        if self.is_closed() {
          return Ok(());
        }
        return (Err(err))
          .map_err_to_unhandleable()
          .context("DevEngine: failed to send EnsureLatestBundleOutput to coordinator")?;
      }

      let Ok(received) = reply_receiver.await else {
        if self.is_closed() {
          return Ok(());
        }
        return Err(anyhow::anyhow!(
          "DevEngine: coordinator closed before responding to EnsureLatestBundleOutput"
        ))?;
      };

      // Wait for the build if one is running or was scheduled
      if let Some(ret) = received {
        // Either a build is ongoing, or a new build was scheduled - wait for it to complete
        ret.future.await;
        if ret.is_ensure_latest_bundle_output_future {
          break;
        }
      } else {
        break;
      }
    }

    Ok(())
  }

  pub fn trigger_full_build(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::TriggerFullBuild)
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send TriggerFullBuild to coordinator")?;

    Ok(())
  }

  /// Client-connect signal (the clientId hello): creates the per-client session with an
  /// empty ship map and the current top-level-evaluated map frozen in. The hello comes from
  /// the dev client script of a page that loaded a served entry chunk, so it doubles as
  /// the entry delivery notification. (A client that loaded an output older than the latest rebuild
  /// gets the newer map; the mismatched entries then read as current copies the client
  /// does not hold — the reload fallback covers that window until the hello carries a
  /// build id.) Reconnects arrive as fresh clientIds, which is the per-client reset.
  pub async fn register_client(&self, client_id: String) {
    let top_level_evaluated = Arc::clone(&*self.dev_context.top_level_evaluated.lock().await);
    self
      .clients
      .lock()
      .await
      .entry(client_id)
      .or_insert_with(|| ClientSession { top_level_evaluated, ..ClientSession::default() });
  }

  /// Client-disconnect signal: drops the session together with any
  /// rendered-but-undelivered payloads addressed to it.
  pub async fn remove_client(&self, client_id: &str) {
    self.clients.lock().await.remove(client_id);
    self.dev_context.pending_payloads.lock().await.retain(|_, p| p.client_id != client_id);
  }

  /// Delivery notification for the payload `filename`. The dev server calls this when
  /// the client reports that it ran the payload: Vite appends a
  /// `__rolldown_runtime__.payloadDelivered(filename)` statement to the end of every
  /// patch and lazy chunk, so the report means every `registerFactory` in the payload
  /// has run. A completed HTTP response is not enough — the bytes may not have
  /// evaluated yet, and a later chunk would then omit a factory the client does not
  /// hold (rolldown/rolldown#10774). Max-merges the pending entry's stamps into that
  /// client's shipped[C] — idempotent, and a late or repeated delivery can never move
  /// the record backwards.
  pub async fn notify_payload_delivered(&self, filename: &str) {
    let Some(pending) = self.dev_context.pending_payloads.lock().await.remove(filename) else {
      return;
    };
    let mut clients = self.clients.lock().await;
    let Some(session) = clients.get_mut(&pending.client_id) else {
      return;
    };
    for (id, stamp) in pending.modules {
      session.shipped.entry(id).and_modify(|e| *e = (*e).max(stamp)).or_insert(stamp);
    }
  }

  /// Compile a lazy entry module and return compiled code.
  ///
  /// This is called when a dynamically imported module is first requested at runtime.
  /// The module was previously stubbed with a proxy, and now we need to compile the
  /// actual module and its dependencies.
  ///
  /// # Arguments
  /// * `proxy_module_id` - The proxy module ID (with ?rolldown-lazy=1 suffix)
  /// * `client_id` - The client ID requesting this compilation
  ///
  /// # Returns
  /// The compiled chunk plus the modules and render-time stamps it carries
  ///
  /// # Panics
  /// - If lazy compilation is not enabled
  /// - If the module is not found
  /// - If compilation fails
  pub async fn compile_lazy_entry(
    &self,
    proxy_module_id: String,
    client_id: String,
  ) -> BuildResult<HmrLazyChunkOutput> {
    self.create_error_if_closed()?;
    let mut bundler = self.bundler.lock().await;

    // Snapshot the ship map `shipped[C]` and the top-level-evaluated map for this client so
    // the compile runs without the clients lock. `ArcStr` keys make the ship-map copy
    // refcount bumps, not string copies; the top-level-evaluated map is shared by `Arc`.
    let (shipped, top_level_evaluated) = self
      .clients
      .lock()
      .await
      .get(&client_id)
      .map(|c| (c.shipped.clone(), Arc::clone(&c.top_level_evaluated)))
      .unwrap_or_default();

    // Mark the proxy module as fetched BEFORE compilation.
    // This changes the content returned by the lazy compilation plugin's load hook
    // from the empty stub to the fetched template, which imports the real module.
    if let Some(lazy_ctx) = &bundler.lazy_compilation_context {
      lazy_ctx.mark_as_fetched(&proxy_module_id);
    }

    // Compile starting from the proxy module.
    // The plugin will return new content (fetched template) that imports the real module,
    // which triggers compilation of the actual module and its dependencies.
    let stamp_table = self.dev_context.stamp_table.lock().await;
    let mut merged = false;
    let mut result = bundler
      .compile_lazy_entry(
        proxy_module_id.clone(),
        &shipped,
        &top_level_evaluated,
        &stamp_table,
        Arc::clone(&self.next_hmr_patch_id),
        &mut merged,
      )
      .await;
    drop(stamp_table);

    if let Ok(output) = &mut result {
      // Record the rendered chunk as pending: the delivery notification for
      // `output.filename` max-merges its stamps into `shipped[C]`. The binding
      // layer drops `carried`, so hand it to the pending entry instead of cloning.
      self
        .dev_context
        .insert_pending_payload(
          output.filename.clone(),
          PendingPayload {
            client_id: client_id.clone(),
            modules: std::mem::take(&mut output.carried),
          },
        )
        .await;
    }

    // A compile that fails after its merge leaves the route in the graph, and a later visit
    // serves the route with no scan. So the assets and the watch list of this compile must
    // still reach the consumer and the coordinator.
    if result.is_ok() || merged {
      // Deliver assets emitted while compiling the lazy entry (e.g. an image
      // imported by the lazy module) before returning the code, so the consumer
      // can register/serve them before the client requests them.
      if let Some(on_additional_assets) = self.dev_context.options.on_additional_assets.as_ref() {
        let mut output = BundleOutput::default();
        bundler.file_emitter.add_additional_files(&mut output.assets, &mut output.warnings);
        if !output.assets.is_empty() {
          on_additional_assets(output);
        }
      }

      // The build output needs no rebuild for the route: pages never load a lazy route from
      // it, because every lazy `import()` calls `requestLazy`. The next build of any kind
      // takes the route from the graph. A lost HMR update is the exception: its rebuild
      // becomes the full build that reloads every client.
      // See internal-docs/lazy-compilation/design.md ("Repeat Visits").
      // The compile added the files it loaded to the watch list of the latest build.
      // A new build can start before the coordinator reads this message, and its list
      // starts empty, so send this list with the message.
      // See internal-docs/dev-engine/implementation.md §15.
      let rebuild = bundler.has_lost_hmr_update();
      self.notify_module_changed(proxy_module_id, Arc::clone(bundler.watch_files()), rebuild);
    }

    result
  }

  /// Notify the coordinator that a lazy compile ran. It always watches the compile's files,
  /// and queues a rebuild only when `rebuild` is true.
  fn notify_module_changed(
    &self,
    module_id: String,
    watch_files: Arc<FxDashSet<ArcStr>>,
    rebuild: bool,
  ) {
    let _ = self.coordinator_sender.unbounded_send(CoordinatorMsg::ModuleChanged {
      module_id,
      watch_files,
      rebuild,
    });
  }

  pub async fn close(&self) -> BuildResult<()> {
    if self.is_closed.swap(true, std::sync::atomic::Ordering::SeqCst) {
      return Ok(());
    }

    // Send close message to coordinator
    self.coordinator_sender.unbounded_send(CoordinatorMsg::Close)
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send Close message to coordinator - coordinator may have already terminated")?;

    // Close the bundler (calls `closeBundle` plugin hook).
    // The bundler lock MUST be released before waiting for the coordinator below.
    // Otherwise we'd deadlock: the coordinator's Close handler waits for any running
    // bundling task to finish, and that task may need to acquire the bundler lock.
    {
      let mut bundler = self.bundler.lock().await;
      bundler.close().await?;
    }

    // Wait for coordinator to close (coordinator handles watcher cleanup)
    let coordinator_state = self.coordinator_state.lock().await;
    if let Some(coordinator_handle) = coordinator_state.handle.clone() {
      if let Err(error) = coordinator_handle.await {
        return Err(anyhow::anyhow!("{error}").into());
      }
    }
    Ok(())
  }

  pub fn is_closed(&self) -> bool {
    self.is_closed.load(std::sync::atomic::Ordering::SeqCst)
  }

  /// Returns a clone of the shared normalized bundler options
  pub async fn bundler_options(&self) -> Arc<NormalizedBundlerOptions> {
    Arc::clone(self.bundler.lock().await.options())
  }

  #[cfg(feature = "testing")]
  pub async fn ensure_task_with_changed_files(
    &self,
    changed_files: FxIndexMap<PathBuf, WatcherChangeKind>,
  ) {
    // Create synthetic file change events to simulate real file system
    // changes. The whole step goes into ONE batch: one event per message
    // would spawn one build per file, while the future awaited below only
    // covers the first, leaving the rest racing the caller's assertions.
    let events = changed_files
      .into_iter()
      .map(|(path, kind)| rolldown_fs_watcher::FsEvent::new(path, kind))
      .collect::<Vec<_>>();

    if !events.is_empty() {
      // Send WatchEvent message to coordinator (simulates real file change)
      // The coordinator will automatically schedule a build via handle_file_changes
      let _ = self.coordinator_sender.unbounded_send(CoordinatorMsg::WatchEvent(events));
    }

    // Send ScheduleBuild to ensure WatchEvent is processed (FIFO),
    // and get the build future to wait on
    let (reply_tx, reply_rx) = futures::channel::oneshot::channel();
    let _ = self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::ScheduleBuildIfStale { reply: reply_tx });

    // Wait for the build that was triggered by the file change
    if let Ok(Some(ret)) = reply_rx.await {
      ret.future.await;
    }
  }

  #[cfg(feature = "testing")]
  pub fn bundler(&self) -> Arc<Mutex<Bundler>> {
    Arc::clone(&self.bundler)
  }

  #[cfg(feature = "testing")]
  pub async fn get_watched_files(&self) -> BuildResult<FxHashSet<String>> {
    self.create_error_if_closed()?;

    let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
    self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::GetWatchedFiles { reply: reply_sender })
      .map_err_to_unhandleable()
      .context(
        "DevEngine: failed to send GetWatchedFiles to coordinator within get_watched_files",
      )?;

    let watched_files = reply_receiver.await.map_err_to_unhandleable().context(
      "DevEngine: coordinator closed before responding to GetWatchedFiles within get_watched_files",
    )?;

    Ok(watched_files)
  }

  #[cfg(feature = "testing")]
  pub async fn create_client_for_testing(&self) {
    let client_session = ClientSession::default();
    // A fixed client ID so HMR steps in tests have a session to compute updates for.
    // Its ship map starts empty and no delivery is ever marked, so every step ships
    // the full affected factory set.
    self.clients.lock().await.insert("rolldown-tests".to_string(), client_session);
  }

  fn create_error_if_closed(&self) -> BuildResult<()> {
    if self.is_closed.load(std::sync::atomic::Ordering::SeqCst) {
      Err(anyhow::anyhow!("Dev engine is closed"))?;
    }
    Ok(())
  }
}

#[derive(Debug, Clone)]
pub struct BundleState {
  /// True for any error state (initial or incremental).
  pub last_build_errored: bool,
  /// The stage of the last incremental failure (`Some` only in
  /// `Failed { .. }`; `None` on success and on `FullBuildFailed`). Lets
  /// the consumer force a full rebuild on access after an `Hmr`-stage
  /// failure — see `internal-docs/dev-engine/implementation.md` §12.
  pub last_error_stage: Option<ErrorStage>,
  pub has_stale_output: bool,
}

impl From<CoordinatorStateSnapshot> for BundleState {
  fn from(snapshot: CoordinatorStateSnapshot) -> Self {
    Self {
      last_build_errored: snapshot.last_build_errored,
      last_error_stage: snapshot.last_error_stage,
      has_stale_output: snapshot.has_stale_output,
    }
  }
}

#[cfg(test)]
mod tests {
  #[cfg(feature = "testing")]
  use std::sync::Arc;

  use rolldown::{BundlerConfig, BundlerOptions};

  #[test]
  fn dev_engine_forces_incremental_build_on() {
    let engine = super::DevEngine::new(
      BundlerConfig::new(BundlerOptions::default(), vec![]),
      crate::DevOptions::default(),
    )
    .unwrap();
    assert!(
      engine.bundler.try_lock().unwrap().options().experimental.is_incremental_build_enabled()
    );
  }

  /// A lazy compile adds the files it loads to the watch list of the build it runs
  /// against. A build that starts before the coordinator handles the compile's
  /// `ModuleChanged` begins with an empty list, and no later build loads those cached
  /// modules again, so the coordinator must still end up watching them.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_compile_files_stay_watched_when_a_build_starts_first() {
    use rolldown::{DevModeOptions, ExperimentalOptions};
    use rolldown_common::ScanMode;

    let dir = std::env::temp_dir().join(format!("rolldown-dev-lazy-watch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The resolver returns real paths, and the temp dir can sit behind a symlink.
    let dir = dunce::canonicalize(&dir).unwrap();
    std::fs::write(dir.join("main.js"), "import('./lazy.js');\n").unwrap();
    std::fs::write(dir.join("lazy.js"), "export { dep } from './dep.js';\n").unwrap();
    std::fs::write(dir.join("dep.js"), "export const dep = 1;\n").unwrap();
    let lazy = dir.join("lazy.js").to_string_lossy().into_owned();
    let dep = dir.join("dep.js").to_string_lossy().into_owned();

    let engine = super::DevEngine::new(
      BundlerConfig::new(
        BundlerOptions {
          input: Some(vec!["./main.js".to_string().into()]),
          cwd: Some(dir.clone()),
          experimental: Some(ExperimentalOptions {
            dev_mode: Some(DevModeOptions { lazy: Some(true), ..Default::default() }),
            ..Default::default()
          }),
          ..Default::default()
        },
        vec![],
      ),
      crate::DevOptions {
        watch: Some(crate::DevWatchOptions {
          enabled: Some(false),
          skip_write: true,
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .unwrap();
    engine.run().await.unwrap();
    let watched = engine.get_watched_files().await.unwrap();
    assert!(!watched.contains(&lazy) && !watched.contains(&dep), "not loaded before the compile");

    engine
      .compile_lazy_entry(format!("{lazy}?rolldown-lazy=1"), "client".to_string())
      .await
      .unwrap();
    // The compile has sent `ModuleChanged`. The coordinator runs on a runtime worker, so it
    // can read the message before or after this build replaces the list; the message carries
    // the compile's list, so the files must end up watched in both orders.
    engine.bundler.lock().await.incremental_generate(ScanMode::Partial(vec![])).await.unwrap();
    engine.ensure_latest_bundle_output().await.unwrap();

    let watched = engine.get_watched_files().await.unwrap();
    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(watched.contains(&lazy) && watched.contains(&dep), "{watched:#?}");
  }

  /// With `RebuildStrategy::Always` one task runs the HMR stage and then a rebuild. The HMR
  /// stage adds the files it loads to the latest build's watch list. The rebuild starts a
  /// new, empty list and does not load those cached modules again, so the coordinator must
  /// still end up watching them.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn hmr_stage_files_stay_watched_when_the_same_task_rebuilds() {
    use rolldown::{DevModeOptions, ExperimentalOptions};
    use rolldown_common::WatcherChangeKind;
    use rolldown_utils::indexmap::FxIndexMap;

    let dir =
      std::env::temp_dir().join(format!("rolldown-dev-hmr-rebuild-watch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The resolver returns real paths, and the temp dir can sit behind a symlink.
    let dir = dunce::canonicalize(&dir).unwrap();
    std::fs::write(dir.join("main.js"), "export const main = 1;\n").unwrap();
    std::fs::write(dir.join("dep.js"), "export const dep = 1;\n").unwrap();
    let main = dir.join("main.js").to_string_lossy().into_owned();
    let dep = dir.join("dep.js").to_string_lossy().into_owned();

    let engine = super::DevEngine::new(
      BundlerConfig::new(
        BundlerOptions {
          input: Some(vec!["./main.js".to_string().into()]),
          cwd: Some(dir.clone()),
          experimental: Some(ExperimentalOptions {
            dev_mode: Some(DevModeOptions::default()),
            ..Default::default()
          }),
          ..Default::default()
        },
        vec![],
      ),
      crate::DevOptions {
        rebuild_strategy: Some(crate::RebuildStrategy::Always),
        watch: Some(crate::DevWatchOptions {
          enabled: Some(false),
          skip_write: true,
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .unwrap();
    engine.run().await.unwrap();
    let watched = engine.get_watched_files().await.unwrap();
    assert!(watched.contains(&main) && !watched.contains(&dep), "{watched:#?}");

    std::fs::write(
      dir.join("main.js"),
      "import { dep } from './dep.js';\nexport const main = dep;\n",
    )
    .unwrap();
    engine
      .ensure_task_with_changed_files(FxIndexMap::from_iter([(
        dir.join("main.js"),
        WatcherChangeKind::Update,
      )]))
      .await;

    let watched = engine.get_watched_files().await.unwrap();
    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(watched.contains(&dep), "{watched:#?}");
  }

  #[cfg(feature = "testing")]
  const LAZY_FILES: &[(&str, &str)] = &[
    ("main.js", "import('./lazy.js');\n"),
    ("lazy.js", "import { dep } from './dep.js';\nexport const value = dep;\n"),
    ("dep.js", "export const dep = 1;\n"),
  ];

  /// A temp dir with `main.js` -> `import('./lazy.js')` -> `dep.js`, and a dev engine with lazy
  /// compilation and sourcemaps on. The caller runs the engine.
  #[cfg(feature = "testing")]
  fn lazy_fixture(name: &str) -> (std::path::PathBuf, super::DevEngine) {
    lazy_fixture_with_files(name, LAZY_FILES)
  }

  /// `files` must include `main.js`, the entry.
  #[cfg(feature = "testing")]
  fn lazy_fixture_with_files(
    name: &str,
    files: &[(&str, &str)],
  ) -> (std::path::PathBuf, super::DevEngine) {
    let (dir, engine, _) = lazy_fixture_reported(name, files, |_| {});
    (dir, engine)
  }

  /// What the engine reported through its callbacks.
  #[cfg(feature = "testing")]
  #[derive(Default)]
  struct Reported {
    /// The `on_output` calls that carried a build.
    builds: std::sync::atomic::AtomicUsize,
    /// Every update that `on_hmr_updates` delivered, for any client.
    hmr_updates: std::sync::Mutex<Vec<rolldown_common::HmrUpdate>>,
    /// The file names that `on_additional_assets` delivered.
    assets: std::sync::Mutex<Vec<String>>,
    /// How often the `resolveId` hook ran. See `CountResolveCalls`.
    resolve_calls: Arc<std::sync::atomic::AtomicUsize>,
  }

  #[cfg(feature = "testing")]
  impl Reported {
    fn builds(&self) -> usize {
      self.builds.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn resolve_calls(&self) -> usize {
      self.resolve_calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn has_full_reload(&self) -> bool {
      self.hmr_updates.lock().unwrap().iter().any(rolldown_common::HmrUpdate::is_full_reload)
    }

    /// Whether a patch announced a change of the module with this stable id.
    fn patched(&self, stable_id: &str) -> bool {
      self.hmr_updates.lock().unwrap().iter().any(|update| {
        matches!(update, rolldown_common::HmrUpdate::Patch(patch)
          if patch.changed_ids.iter().any(|id| id == stable_id))
      })
    }
  }

  /// Counts the `resolveId` calls. A lazy compile resolves only when it scans: user plugins
  /// never see the proxy itself, but they resolve the call that the proxy replaced.
  #[cfg(feature = "testing")]
  #[derive(Debug)]
  struct CountResolveCalls(Arc<std::sync::atomic::AtomicUsize>);

  #[cfg(feature = "testing")]
  impl rolldown::plugin::Plugin for CountResolveCalls {
    fn name(&self) -> std::borrow::Cow<'static, str> {
      "count-resolve-calls".into()
    }

    fn register_hook_usage(&self) -> rolldown::plugin::HookUsage {
      rolldown::plugin::HookUsage::ResolveId
    }

    async fn resolve_id(
      &self,
      _ctx: &rolldown::plugin::PluginContext,
      _args: &rolldown::plugin::HookResolveIdArgs<'_>,
    ) -> rolldown::plugin::HookResolveIdReturn {
      self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      Ok(None)
    }
  }

  /// Like `lazy_fixture_with_files`. `configure` can change the bundler options, and the
  /// returned `Reported` records what the engine reported.
  #[cfg(feature = "testing")]
  fn lazy_fixture_reported(
    name: &str,
    files: &[(&str, &str)],
    configure: impl FnOnce(&mut BundlerOptions),
  ) -> (std::path::PathBuf, super::DevEngine, Arc<Reported>) {
    use rolldown::{DevModeOptions, ExperimentalOptions};
    use rolldown_common::SourceMapType;

    let dir = std::env::temp_dir().join(format!("rolldown-dev-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // The resolver returns real paths, and the temp dir can sit behind a symlink.
    let dir = dunce::canonicalize(&dir).unwrap();
    for (file, content) in files {
      std::fs::write(dir.join(file), content).unwrap();
    }

    let mut options = BundlerOptions {
      input: Some(vec!["./main.js".to_string().into()]),
      cwd: Some(dir.clone()),
      sourcemap: Some(SourceMapType::File),
      experimental: Some(ExperimentalOptions {
        dev_mode: Some(DevModeOptions { lazy: Some(true), ..Default::default() }),
        ..Default::default()
      }),
      ..Default::default()
    };
    configure(&mut options);
    let reported = Arc::new(Reported::default());
    let on_output = Arc::clone(&reported);
    let on_hmr_updates = Arc::clone(&reported);
    let on_additional_assets = Arc::clone(&reported);
    let engine = super::DevEngine::new(
      BundlerConfig::new(
        options,
        vec![rolldown::plugin::Plugin::new_shared(CountResolveCalls(Arc::clone(
          &reported.resolve_calls,
        )))],
      ),
      crate::DevOptions {
        on_output: Some(Arc::new(move |result| {
          if result.is_ok() {
            on_output.builds.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
          }
        })),
        on_hmr_updates: Some(Arc::new(move |result| {
          if let Ok((updates, _)) = result {
            on_hmr_updates
              .hmr_updates
              .lock()
              .unwrap()
              .extend(updates.into_iter().map(|client_update| client_update.update));
          }
        })),
        on_additional_assets: Some(Arc::new(move |output| {
          on_additional_assets
            .assets
            .lock()
            .unwrap()
            .extend(output.assets.iter().map(|asset| asset.filename().to_owned()));
        })),
        watch: Some(crate::DevWatchOptions {
          enabled: Some(false),
          skip_write: true,
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .unwrap();
    (dir, engine, reported)
  }

  #[cfg(feature = "testing")]
  fn lazy_proxy_id(dir: &std::path::Path) -> String {
    format!("{}?rolldown-lazy=1", dir.join("lazy.js").to_string_lossy())
  }

  #[cfg(feature = "testing")]
  fn changed(
    dir: &std::path::Path,
    file: &str,
    kind: rolldown_common::WatcherChangeKind,
  ) -> rolldown_utils::indexmap::FxIndexMap<std::path::PathBuf, rolldown_common::WatcherChangeKind>
  {
    rolldown_utils::indexmap::FxIndexMap::from_iter([(dir.join(file), kind)])
  }

  /// Each compile gets its own file name. Mask it, so two payloads compare by content.
  #[cfg(feature = "testing")]
  fn mask_payload_name(output: &rolldown_common::HmrLazyChunkOutput) -> (String, Option<String>) {
    let mask = |text: &str| text.replace(&output.filename, "lazy_compile_N.js");
    (mask(&output.code), output.sourcemap.as_deref().map(mask))
  }

  /// A first visit merges the route into the graph and queues no build: no page loads a lazy
  /// route from the build output. The coordinator still watches the route's files, so an edit
  /// of one gives an HMR update.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_first_visit_queues_no_rebuild() {
    use rolldown_common::WatcherChangeKind;

    let (dir, engine, reported) = lazy_fixture_reported("lazy-first-visit", LAZY_FILES, |_| {});
    engine.run().await.unwrap();
    engine.create_client_for_testing().await;

    engine.compile_lazy_entry(lazy_proxy_id(&dir), "c1".to_string()).await.unwrap();
    let state = engine.get_bundle_state().await.unwrap();
    let watched = engine.get_watched_files().await.unwrap();
    let builds = reported.builds();
    std::fs::write(dir.join("dep.js"), "export const dep = 2;\n").unwrap();
    engine.ensure_task_with_changed_files(changed(&dir, "dep.js", WatcherChangeKind::Update)).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(!state.has_stale_output, "the first visit leaves the build output current");
    assert_eq!(builds, 1, "only the initial build ran");
    assert!(watched.contains(dir.join("dep.js").to_string_lossy().as_ref()), "{watched:#?}");
    assert!(reported.patched("dep.js"), "the edit of a route file gives an update");
  }

  /// A repeat visit of a fetched route does not scan: the graph already holds the route. It
  /// serves the same code and sourcemap as the first visit, even though `lazy.js` changed on
  /// disk with no watch event.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_repeat_visit_skips_the_scan_and_the_rebuild() {
    let (dir, engine, reported) = lazy_fixture_reported("lazy-repeat-visit", LAZY_FILES, |_| {});
    engine.run().await.unwrap();
    let proxy = lazy_proxy_id(&dir);

    let first = engine.compile_lazy_entry(proxy.clone(), "c1".to_string()).await.unwrap();
    let resolves_after_first = reported.resolve_calls();
    std::fs::write(dir.join("lazy.js"), "export const value = 'unseen';\n").unwrap();
    // A client id the engine has never seen holds nothing, like a new tab.
    let repeat = engine.compile_lazy_entry(proxy, "c2".to_string()).await.unwrap();
    let state_after_repeat = engine.get_bundle_state().await.unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(reported.resolve_calls(), resolves_after_first, "the repeat visit scans");
    assert!(!state_after_repeat.has_stale_output, "no visit queues a rebuild");
    assert!(first.sourcemap.is_some());
    assert_eq!(mask_payload_name(&repeat), mask_payload_name(&first));
  }

  /// A failed first compile never merges the fetched proxy, so the next compile scans again
  /// and sees the fixed file. The proxy is read again, but it is not a lost update. The failed
  /// scan queued the proxy for a rescan, and the retry takes it out of the queue, so a third
  /// visit does not scan.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_compile_scans_again_after_a_failed_first_compile() {
    let (dir, engine, reported) =
      lazy_fixture_reported("lazy-retry-first-compile", LAZY_FILES, |_| {});
    engine.run().await.unwrap();
    engine.create_client_for_testing().await;
    let proxy = lazy_proxy_id(&dir);

    std::fs::write(dir.join("dep.js"), "export const dep = ;\n").unwrap();
    let failed = engine.compile_lazy_entry(proxy.clone(), "c1".to_string()).await;
    std::fs::write(dir.join("dep.js"), "export const dep = 'fixed';\n").unwrap();
    let retry = engine.compile_lazy_entry(proxy.clone(), "c1".to_string()).await;
    let state_after_retry = engine.get_bundle_state().await.unwrap();
    let resolves_after_retry = reported.resolve_calls();
    let third = engine.compile_lazy_entry(proxy, "c2".to_string()).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    failed.expect_err("the first compile hits the broken file");
    let retry = retry.unwrap();
    assert!(retry.code.contains("'fixed'") || retry.code.contains("\"fixed\""), "{}", retry.code);
    assert!(!state_after_retry.has_stale_output, "the retry queues no rebuild");
    assert!(!reported.has_full_reload());
    third.unwrap();
    assert_eq!(reported.resolve_calls(), resolves_after_retry, "the third visit scans");
  }

  /// A failed first visit of `b.js` queues its proxy for a rescan. A visit of `c.js` scans only
  /// its own proxy, so the broken `b.js` does not fail it.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_visit_is_served_after_a_failed_visit_of_another_route() {
    let (dir, engine, reported) = lazy_fixture_reported(
      "lazy-visit-after-failed-route",
      &[
        ("main.js", "import('./b.js');\nimport('./c.js');\n"),
        ("b.js", "export const b = ;\n"),
        ("c.js", "export const c = 'c-value';\n"),
      ],
      |_| {},
    );
    engine.run().await.unwrap();
    engine.create_client_for_testing().await;
    let proxy = |file: &str| format!("{}?rolldown-lazy=1", dir.join(file).to_string_lossy());

    let failed = engine.compile_lazy_entry(proxy("b.js"), "c1".to_string()).await;
    let other = engine.compile_lazy_entry(proxy("c.js"), "c1".to_string()).await;
    let state = engine.get_bundle_state().await.unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    failed.expect_err("b.js is broken");
    let other = other.unwrap();
    assert!(other.code.contains("c-value"), "{}", other.code);
    assert!(!state.has_stale_output);
    assert!(!reported.has_full_reload());
  }

  /// A compile that fails after it merged the route keeps the route in the graph. It still
  /// delivers the route's asset and watches the route's files. The repeat visit serves the
  /// route with no rebuild.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_compile_failed_past_its_merge_keeps_the_route() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use rolldown_common::{SourceMapIgnoreList, WatcherChangeKind};
    use rustc_hash::FxHashMap;

    let fail = Arc::new(AtomicBool::new(false));
    let fail_in_callback = Arc::clone(&fail);
    let (dir, engine, reported) = lazy_fixture_reported(
      "lazy-failed-past-merge",
      &[
        ("main.js", "import('./lazy.js');\n"),
        (
          "lazy.js",
          "import { dep } from './dep.js';\nimport image from './image.png';\nexport const value = [dep, image];\n",
        ),
        ("dep.js", "export const dep = 1;\n"),
        ("image.png", "not really a png"),
      ],
      move |options| {
        options.module_types =
          Some(FxHashMap::from_iter([(".png".to_string(), rolldown_common::ModuleType::Asset)]));
        // The sourcemap step runs after the merge.
        options.sourcemap_ignore_list =
          Some(SourceMapIgnoreList::new(Arc::new(move |sources, _| {
            let fail = fail_in_callback.load(Ordering::Relaxed);
            Box::pin(async move {
              if fail {
                anyhow::bail!("sourcemapIgnoreList failed");
              }
              Ok(vec![false; sources.len()])
            })
          })));
      },
    );
    engine.run().await.unwrap();
    engine.create_client_for_testing().await;
    let proxy = lazy_proxy_id(&dir);

    fail.store(true, Ordering::Relaxed);
    let failed = engine.compile_lazy_entry(proxy.clone(), "c1".to_string()).await;
    let assets_after_failure = reported.assets.lock().unwrap().clone();
    let watched = engine.get_watched_files().await.unwrap();
    fail.store(false, Ordering::Relaxed);
    let repeat = engine.compile_lazy_entry(proxy, "c2".to_string()).await;
    let state_after_repeat = engine.get_bundle_state().await.unwrap();
    std::fs::write(dir.join("dep.js"), "export const dep = 2;\n").unwrap();
    engine.ensure_task_with_changed_files(changed(&dir, "dep.js", WatcherChangeKind::Update)).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    failed.expect_err("the sourcemap step fails");
    assert!(
      assets_after_failure.iter().any(|name| name.ends_with(".png")),
      "{assets_after_failure:#?}"
    );
    assert!(watched.contains(dir.join("dep.js").to_string_lossy().as_ref()), "{watched:#?}");
    let repeat = repeat.unwrap();
    assert!(repeat.code.contains("lazy.js"), "{}", repeat.code);
    assert!(!state_after_repeat.has_stale_output, "the repeat visit queues no rebuild");
    assert_eq!(reported.builds(), 1, "only the initial build ran");
    assert!(reported.patched("dep.js"), "the edit of a route file gives an update");
  }

  /// `a.js` calls `import('./utils.js')`, and no client visited it, so its proxy is the stub
  /// with no `import()`. `b.js` imports `utils.js` statically, so `requestLazy` runs it with no
  /// compile. The client graph links `a.js` to `utils.js`, so an edit of `utils.js` re-runs
  /// `a.js`, and the patch must carry its factory.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn hmr_patch_carries_the_import_importer_of_an_unfetched_proxy() {
    use std::sync::{Arc, atomic::AtomicU32};

    use rolldown_common::{ClientHmrInput, HmrStampTable, HmrUpdate, WatcherChangeKind};
    use rolldown_utils::indexmap::FxIndexMap;
    use rustc_hash::FxHashMap;

    let (dir, engine) = lazy_fixture_with_files(
      "hmr-unfetched-proxy-importer",
      &[
        ("main.js", "import './a.js';\nimport './b.js';\n"),
        ("a.js", "export const load = () => import('./utils.js');\nimport.meta.hot.accept();\n"),
        (
          "b.js",
          "import { u } from './utils.js';\nexport const value = u;\nimport.meta.hot.accept();\n",
        ),
        ("utils.js", "export const u = 1;\n"),
      ],
    );
    engine.run().await.unwrap();

    std::fs::write(dir.join("utils.js"), "export const u = 2;\n").unwrap();
    let shipped = FxHashMap::default();
    let top_level_evaluated = FxHashMap::default();
    let updates = engine
      .bundler
      .lock()
      .await
      .compute_hmr_update_for_file_changes(
        &FxIndexMap::from_iter([(
          dir.join("utils.js").to_string_lossy().into_owned(),
          WatcherChangeKind::Update,
        )]),
        &[ClientHmrInput {
          client_id: "c1",
          shipped: &shipped,
          top_level_evaluated: &top_level_evaluated,
        }],
        &mut HmrStampTable::default(),
        Arc::new(AtomicU32::new(0)),
        false,
        false,
      )
      .await
      .unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    let HmrUpdate::Patch(patch) = &updates[0].update else {
      panic!("expected a patch, got {:?}", updates[0].update);
    };
    let mut carried = patch.carried.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>();
    carried.sort_unstable();
    assert_eq!(carried, ["a.js", "b.js", "utils.js"]);
  }

  /// A failed HMR scan of `other.js` queues it for a rescan, and the graph keeps its last good
  /// code. Lazy visits do not retry it: a repeat visit and a first visit of another route are
  /// both served.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_visits_are_served_while_an_unrelated_file_is_broken() {
    use rolldown_common::WatcherChangeKind;

    let (dir, engine) = lazy_fixture_with_files(
      "lazy-visits-unrelated-broken",
      &[
        ("main.js", "import './other.js';\nimport('./lazy.js');\nimport('./second.js');\n"),
        ("other.js", "export const other = 1;\n"),
        ("lazy.js", "import { dep } from './dep.js';\nexport const value = dep;\n"),
        ("dep.js", "export const dep = 1;\n"),
        ("second.js", "export const second = 'second-value';\n"),
      ],
    );
    engine.run().await.unwrap();
    engine.create_client_for_testing().await;
    let proxy = lazy_proxy_id(&dir);

    engine.compile_lazy_entry(proxy.clone(), "c1".to_string()).await.unwrap();
    std::fs::write(dir.join("other.js"), "export const other = ;\n").unwrap();
    engine
      .ensure_task_with_changed_files(changed(&dir, "other.js", WatcherChangeKind::Update))
      .await;
    let state_after_break = engine.get_bundle_state().await.unwrap();
    let repeat = engine.compile_lazy_entry(proxy, "c2".to_string()).await;
    let second = engine
      .compile_lazy_entry(
        format!("{}?rolldown-lazy=1", dir.join("second.js").to_string_lossy()),
        "c2".to_string(),
      )
      .await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(state_after_break.last_build_errored, "the HMR scan of the broken file fails");
    let repeat = repeat.unwrap();
    assert!(repeat.code.contains("lazy.js"), "{}", repeat.code);
    let second = second.unwrap();
    assert!(second.code.contains("second-value"), "{}", second.code);
  }

  /// A failed HMR scan queues `other.js`. A failed visit of `b.js` adds its own proxy to the
  /// queue and keeps `other.js` there. So the next HMR task retries `other.js`, although no
  /// event announced its fix.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_failed_visit_keeps_the_other_queued_files() {
    use rolldown_common::WatcherChangeKind;

    let (dir, engine, reported) = lazy_fixture_reported(
      "lazy-failed-visit-keeps-queue",
      &[
        ("main.js", "import './other.js';\nimport './x.js';\nimport('./b.js');\n"),
        ("other.js", "export const other = 1;\n"),
        ("x.js", "export const x = 1;\n"),
        ("b.js", "export const b = ;\n"),
      ],
      |_| {},
    );
    engine.run().await.unwrap();
    engine.create_client_for_testing().await;

    std::fs::write(dir.join("other.js"), "export const other = ;\n").unwrap();
    engine
      .ensure_task_with_changed_files(changed(&dir, "other.js", WatcherChangeKind::Update))
      .await;
    let state_after_break = engine.get_bundle_state().await.unwrap();
    let failed = engine
      .compile_lazy_entry(
        format!("{}?rolldown-lazy=1", dir.join("b.js").to_string_lossy()),
        "c1".to_string(),
      )
      .await;
    // Fixed with no watch event: only the queue can bring `other.js` to the next HMR task.
    std::fs::write(dir.join("other.js"), "export const other = 'fixed';\n").unwrap();
    std::fs::write(dir.join("b.js"), "export const b = 1;\n").unwrap();
    std::fs::write(dir.join("x.js"), "export const x = 2;\n").unwrap();
    engine.ensure_task_with_changed_files(changed(&dir, "x.js", WatcherChangeKind::Update)).await;
    let state = engine.get_bundle_state().await.unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(state_after_break.last_build_errored, "the HMR scan of the broken file fails");
    failed.expect_err("b.js is broken");
    assert!(!state.last_build_errored);
    assert!(reported.patched("other.js"), "the HMR task retries the queued other.js");
  }

  /// An HMR update fails after its merge, so it is lost. A repeat visit is served from the
  /// graph, and its rebuild becomes the full build that reloads every client.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_visit_with_a_lost_update_reloads_every_client() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use rolldown_common::{SourceMapIgnoreList, WatcherChangeKind};

    let fail = Arc::new(AtomicBool::new(false));
    let fail_in_callback = Arc::clone(&fail);
    let (dir, engine, reported) =
      lazy_fixture_reported("lazy-visit-lost-update", LAZY_FILES, move |options| {
        // The HMR patch runs the sourcemap step after its merge.
        options.sourcemap_ignore_list =
          Some(SourceMapIgnoreList::new(Arc::new(move |sources, _| {
            let fail = fail_in_callback.load(Ordering::Relaxed);
            Box::pin(async move {
              if fail {
                anyhow::bail!("sourcemapIgnoreList failed");
              }
              Ok(vec![false; sources.len()])
            })
          })));
      });
    engine.run().await.unwrap();
    engine.create_client_for_testing().await;
    let proxy = lazy_proxy_id(&dir);

    engine.compile_lazy_entry(proxy.clone(), "c1".to_string()).await.unwrap();
    fail.store(true, Ordering::Relaxed);
    std::fs::write(dir.join("dep.js"), "export const dep = 'lost';\n").unwrap();
    engine.ensure_task_with_changed_files(changed(&dir, "dep.js", WatcherChangeKind::Update)).await;
    let state_after_loss = engine.get_bundle_state().await.unwrap();
    fail.store(false, Ordering::Relaxed);
    let repeat = engine.compile_lazy_entry(proxy, "c2".to_string()).await;
    // Wait for the task that the visit queued.
    engine.ensure_task_with_changed_files(rolldown_utils::indexmap::FxIndexMap::default()).await;
    let state_after_visit = engine.get_bundle_state().await.unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(state_after_loss.last_build_errored, "the HMR patch fails after its merge");
    let repeat = repeat.unwrap();
    assert!(repeat.code.contains("'lost'") || repeat.code.contains("\"lost\""), "{}", repeat.code);
    assert!(reported.has_full_reload(), "every client reloads");
    assert_eq!(reported.builds(), 2, "the initial build and the full build");
    assert!(!state_after_visit.last_build_errored && !state_after_visit.has_stale_output);
  }

  /// Deleting a fetched route's file rescans the proxy in the HMR stage, and that scan fails
  /// to resolve the file. The proxy is then queued for a rescan, so the next compile scans and
  /// reports the missing file instead of serving the deleted module from the graph.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_repeat_visit_reports_a_deleted_route_file() {
    use rolldown_common::WatcherChangeKind;

    let (dir, engine) = lazy_fixture("lazy-repeat-visit-deleted");
    engine.run().await.unwrap();
    engine.create_client_for_testing().await;
    let proxy = lazy_proxy_id(&dir);

    engine.compile_lazy_entry(proxy.clone(), "c1".to_string()).await.unwrap();
    std::fs::remove_file(dir.join("lazy.js")).unwrap();
    engine
      .ensure_task_with_changed_files(changed(&dir, "lazy.js", WatcherChangeKind::Delete))
      .await;
    let repeat = engine.compile_lazy_entry(proxy, "c2".to_string()).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    let errors = format!("{:?}", repeat.expect_err("the deleted file must not be served"));
    assert!(errors.contains("UNRESOLVED_IMPORT") && errors.contains("lazy.js"), "{errors}");
  }
}
