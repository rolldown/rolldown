use std::sync::{
  Arc,
  atomic::{AtomicBool, AtomicU32},
};

use anyhow::Context;
use async_lock::Mutex;
use futures::channel::mpsc::unbounded;
use futures::{FutureExt, future::Shared};
#[cfg(feature = "testing")]
use rolldown_common::WatcherChangeKind;
use rolldown_common::{HmrLazyChunkOutput, HmrStampTable};
use rolldown_error::{BuildResult, ResultExt};
use rolldown_fs_watcher::{FsWatcher, FsWatcherConfig, FsWatcherExt, NoopFsWatcher};
use rolldown_utils::futures::try_spawn;
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
type PendingCoordinatorFuture = PinBoxSendStaticFuture<()>;
type CoordinatorTaskFuture = Shared<PinBoxSendStaticFuture<CoordinatorTaskResult>>;

pub struct CoordinatorState {
  coordinator: Option<PendingCoordinatorFuture>,
  handle: Option<CoordinatorTaskFuture>,
}

impl CoordinatorState {
  /// Start the retained coordinator future. When submission fails (e.g. the
  /// async runtime rejected the spawn), the coordinator is retained so a later
  /// call can retry after a runtime restart.
  fn try_start<E>(
    &mut self,
    start: impl FnOnce(
      PendingCoordinatorFuture,
    ) -> Result<CoordinatorTaskFuture, (E, PendingCoordinatorFuture)>,
  ) -> Result<(), E> {
    let Some(coordinator) = self.coordinator.take() else {
      return Ok(());
    };

    match start(coordinator) {
      Ok(handle) => {
        self.handle = Some(handle);
        Ok(())
      }
      Err((error, coordinator)) => {
        self.coordinator = Some(coordinator);
        Err(error)
      }
    }
  }
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
}

impl DevEngine {
  pub fn new(config: BundlerConfig, options: DevOptions) -> BuildResult<Self> {
    // Build the bundler from config
    let bundler = BundlerBuilder::default()
      .with_options(config.options)
      .with_plugins(config.plugins)
      .build()?;

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
      poll_interval: ctx.options.poll_interval,
      debounce_delay: ctx.options.debounce_duration,
      compare_contents_for_polling: ctx.options.compare_contents_for_polling,
      debounce_tick_rate: ctx.options.debounce_tick_rate,
      use_polling: ctx.options.use_polling,
      use_debounce: ctx.options.use_debounce,
    };

    let event_handler = BundleCoordinator::create_watcher_event_handler(coordinator_tx.clone());

    let watcher = if ctx.options.disable_watcher {
      NoopFsWatcher::with_config(event_handler, watcher_config)?.into_dyn_fs_watcher()
    } else {
      rolldown_fs_watcher::create_fs_watcher(event_handler, watcher_config)?
    };

    let coordinator = BundleCoordinator::new(
      Arc::clone(&bundler),
      Arc::clone(&ctx),
      coordinator_rx,
      watcher,
      Arc::clone(&next_hmr_patch_id),
    );
    let coordinator = Box::pin(coordinator.run()) as PendingCoordinatorFuture;

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
    })
  }

  pub async fn run(&self) -> BuildResult<()> {
    let mut coordinator_state = self.coordinator_state.lock().await;

    if coordinator_state.coordinator.is_none() {
      // The coordinator is already running.
      return Ok(());
    }

    // Spawn the coordinator. On submission failure (e.g. the async runtime is
    // shut down) the coordinator is retained so a retry after a runtime
    // restart can still start it.
    let start_result = coordinator_state.try_start(|coordinator| match try_spawn(coordinator) {
      Ok(join_handle) => {
        let coordinator_handle = Box::pin(async move {
          join_handle.await.map_err(|error| {
            Arc::<str>::from(format!("DevEngine coordinator task failed: {error}"))
          })
        }) as PinBoxSendStaticFuture<CoordinatorTaskResult>;
        Ok(coordinator_handle.shared())
      }
      Err((error, coordinator)) => Err((error, coordinator)),
    });
    if let Err(error) = start_result {
      return Err(anyhow::anyhow!("DevEngine coordinator task submission failed: {error}").into());
    }
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

  // Ensure there's latest bundle output available for browser loading/reloading scenarios
  pub async fn ensure_latest_bundle_output(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    let mut loop_count = 0u32;
    loop {
      loop_count += 1;
      if loop_count > 100 {
        if cfg!(debug_assertions) {
          panic!(
            "[DevEngine] ensure_latest_bundle_output has looped {loop_count} times, something is definitely wrong",
          );
        } else {
          tracing::warn!(
            "[DevEngine] ensure_latest_bundle_output has looped {loop_count} times, something might be wrong",
          );
        }
        break;
      }
      let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
      self
        .coordinator_sender
        .unbounded_send(CoordinatorMsg::EnsureLatestBundleOutput { reply: reply_sender })
        .map_err_to_unhandleable()
        .context("DevEngine: failed to send EnsureLatestBundleOutput to coordinator")?;

      let received = reply_receiver
        .await
        .map_err_to_unhandleable()
        .context("DevEngine: coordinator closed before responding to EnsureLatestBundleOutput")?;

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
  /// the runtime inside the entry chunk, so it doubles as the entry delivery
  /// notification. (A client that loaded an output older than the latest rebuild
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

  /// Delivery notification from the serving middleware: the response for `filename`
  /// completed. Max-merges the pending entry's stamps into that client's shipped[C] —
  /// idempotent, and a late or repeated delivery can never move the record backwards.
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
    // from a stub (fetches via /lazy endpoint) to actual code that imports the real module.
    if let Some(lazy_ctx) = &bundler.lazy_compilation_context {
      lazy_ctx.mark_as_fetched(&proxy_module_id);
    }

    // Compile starting from the proxy module.
    // The plugin will return new content (fetched template) that imports the real module,
    // which triggers compilation of the actual module and its dependencies.
    let stamp_table = self.dev_context.stamp_table.lock().await;
    let mut result = bundler
      .compile_lazy_entry(
        proxy_module_id.clone(),
        &client_id,
        &shipped,
        &top_level_evaluated,
        &stamp_table,
        Arc::clone(&self.next_hmr_patch_id),
      )
      .await;
    drop(stamp_table);

    if let Ok(output) = &mut result {
      // Record the rendered chunk as pending: the delivery notification
      // max-merges its stamps into `shipped[C]` when the serving middleware
      // sees the response for `output.filename` complete. The binding layer
      // drops `carried`, so hand it to the pending entry instead of cloning.
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

      // Notify that the proxy module has changed so build output gets updated.
      // This ensures future page loads get the fetched template directly.
      self.notify_module_changed(proxy_module_id);
    }

    result
  }

  /// Notify the coordinator that a module has changed programmatically.
  /// This triggers a rebuild to update the build output.
  fn notify_module_changed(&self, module_id: String) {
    let _ = self.coordinator_sender.unbounded_send(CoordinatorMsg::ModuleChanged { module_id });
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
    let mut coordinator_state = self.coordinator_state.lock().await;
    // Drop a coordinator future that was constructed but never spawned (e.g.
    // `run()` failed to submit it onto a stopped runtime): its fs watcher and
    // receiver would otherwise linger until the engine itself is dropped, rather
    // than being released here at `close()`. When the coordinator was spawned,
    // `try_start` already took it (leaving `None`), so this is a no-op.
    coordinator_state.coordinator = None;
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
      .map(|(path, event)| {
        let notify_event = notify::Event {
          kind: if event == WatcherChangeKind::Delete {
            notify::EventKind::Remove(notify::event::RemoveKind::Any)
          } else {
            notify::EventKind::Modify(notify::event::ModifyKind::Data(
              notify::event::DataChange::Any,
            ))
          },
          paths: vec![path],
          attrs: notify::event::EventAttributes::default(),
        };
        rolldown_fs_watcher::FsEvent { detail: notify_event, time: std::time::Instant::now() }
      })
      .collect::<Vec<_>>();

    if !events.is_empty() {
      // Send WatchEvent message to coordinator (simulates real file change)
      // The coordinator will automatically schedule a build via handle_file_changes
      let _ = self.coordinator_sender.unbounded_send(CoordinatorMsg::WatchEvent(Ok(events)));
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
