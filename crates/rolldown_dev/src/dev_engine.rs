use std::sync::{
  Arc,
  atomic::{AtomicBool, AtomicU32},
};

use anyhow::Context;
use futures::{FutureExt, future::Shared};
use rolldown_common::ClientHmrUpdate;
#[cfg(feature = "testing")]
use rolldown_common::WatcherChangeKind;
use rolldown_error::{BatchedBuildDiagnostic, BuildResult, ResultExt};
use rolldown_fs_watcher::{FsWatcher, FsWatcherConfig, FsWatcherExt, NoopFsWatcher};
use rolldown_utils::futures::spawn;
#[cfg(feature = "testing")]
use rustc_hash::FxHashSet;
use tokio::sync::{Mutex, mpsc::unbounded_channel};

use rolldown::{Bundler, BundlerBuilder, BundlerConfig, NormalizedBundlerOptions};

use crate::{
  BundleOutput, DevOptions, SharedClients,
  bundle_coordinator::BundleCoordinator,
  dev_context::{DevContext, PinBoxSendStaticFuture},
  normalize_dev_options,
  type_aliases::CoordinatorSender,
  types::{
    coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot,
    error_stage::ErrorStage,
  },
};

type DevEngineCloseResult = Result<(), Arc<str>>;
type DevEngineCloseFuture = Shared<PinBoxSendStaticFuture<DevEngineCloseResult>>;
type CoordinatorTaskResult = Result<(), Arc<str>>;
type CoordinatorTaskFuture = Shared<PinBoxSendStaticFuture<CoordinatorTaskResult>>;

#[cfg(feature = "testing")]
use crate::ClientSession;
#[cfg(feature = "testing")]
use rolldown_utils::indexmap::FxIndexMap;
#[cfg(feature = "testing")]
use std::path::PathBuf;

pub struct CoordinatorState {
  coordinator: Option<BundleCoordinator>,
  handle: Option<CoordinatorTaskFuture>,
}

pub struct DevEngine {
  coordinator_sender: CoordinatorSender,
  bundler: Arc<Mutex<Bundler>>,
  /// Shared dev context, kept so out-of-coordinator entry points (e.g.
  /// `compile_lazy_entry`) can reach `options.on_additional_assets`.
  dev_context: Arc<DevContext>,
  coordinator_state: Arc<Mutex<CoordinatorState>>,
  close_future: std::sync::Mutex<Option<DevEngineCloseFuture>>,
  pub clients: SharedClients,
  is_closed: AtomicBool,
  /// Counter for HMR patch IDs used by invalidate() method
  next_invalidate_patch_id: Arc<AtomicU32>,
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

    let (coordinator_tx, coordinator_rx) = unbounded_channel::<CoordinatorMsg>();

    let clients = SharedClients::default();

    let ctx = Arc::new(DevContext {
      options: normalized_options,
      coordinator_tx: coordinator_tx.clone(),
      clients: Arc::clone(&clients),
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

    let coordinator =
      BundleCoordinator::new(Arc::clone(&bundler), Arc::clone(&ctx), coordinator_rx, watcher);

    Ok(Self {
      coordinator_sender: coordinator_tx,
      bundler,
      dev_context: Arc::clone(&ctx),
      coordinator_state: Arc::new(Mutex::new(CoordinatorState {
        coordinator: Some(coordinator),
        handle: None,
      })),
      close_future: std::sync::Mutex::new(None),
      clients,
      is_closed: AtomicBool::new(false),
      next_invalidate_patch_id: Arc::new(AtomicU32::new(0)),
    })
  }

  pub async fn run(&self) -> BuildResult<()> {
    let mut coordinator_state = self.coordinator_state.lock().await;

    if coordinator_state.coordinator.is_none() {
      // The coordinator is already running.
      return Ok(());
    }

    // Spawn the coordinator
    if let Some(coordinator) = coordinator_state.coordinator.take() {
      let join_handle = spawn(coordinator.run());
      let coordinator_handle = Box::pin(async move {
        join_handle
          .await
          .map_err(|error| Arc::<str>::from(format!("DevEngine coordinator task failed: {error}")))
      }) as PinBoxSendStaticFuture<CoordinatorTaskResult>;
      coordinator_state.handle = Some(coordinator_handle.shared());
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
        return Err(anyhow::anyhow!("{error}"))?;
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

    let (reply_sender, reply_receiver) = tokio::sync::oneshot::channel();
    if let Err(err) = self.coordinator_sender.send(CoordinatorMsg::GetState { reply: reply_sender })
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

    let (reply_sender, reply_receiver) = tokio::sync::oneshot::channel();
    self
      .coordinator_sender
      .send(CoordinatorMsg::GetState { reply: reply_sender })
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
      let (reply_sender, reply_receiver) = tokio::sync::oneshot::channel();
      self
        .coordinator_sender
        .send(CoordinatorMsg::EnsureLatestBundleOutput { reply: reply_sender })
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
      .send(CoordinatorMsg::TriggerFullBuild)
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send TriggerFullBuild to coordinator")?;

    Ok(())
  }

  pub async fn invalidate(
    &self,
    caller: String,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    self.create_error_if_closed()?;
    let mut bundler = self.bundler.lock().await;

    // Use bundler directly for invalidation (avoid message roundtrip)
    let mut updates = Vec::new();
    let clients = self.clients.lock().await;
    for (client_key, client) in clients.iter() {
      let update = bundler
        .compute_update_for_calling_invalidate(
          caller.clone(),
          first_invalidated_by.clone(),
          client_key,
          &client.executed_modules,
          Arc::clone(&self.next_invalidate_patch_id),
        )
        .await?;
      updates.push(ClientHmrUpdate { client_id: client_key.clone(), update });
    }

    Ok(updates)
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
  /// The compiled JavaScript code as a string
  ///
  /// # Panics
  /// - If lazy compilation is not enabled
  /// - If the module is not found
  /// - If compilation fails
  pub async fn compile_lazy_entry(
    &self,
    proxy_module_id: String,
    client_id: String,
  ) -> BuildResult<String> {
    self.create_error_if_closed()?;
    let mut bundler = self.bundler.lock().await;

    // Get executed modules for this client
    let executed_modules = self
      .clients
      .lock()
      .await
      .get(&client_id)
      .map(|c| c.executed_modules.clone())
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
    let result = bundler
      .compile_lazy_entry(
        proxy_module_id.clone(),
        &client_id,
        &executed_modules,
        Arc::clone(&self.next_invalidate_patch_id),
      )
      .await;

    if result.is_ok() {
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
    let _ = self.coordinator_sender.send(CoordinatorMsg::ModuleChanged { module_id });
  }

  pub async fn close(&self) -> BuildResult<()> {
    // Reject new work immediately, independently of how long terminal cleanup
    // takes or whether it eventually fails.
    self.is_closed.store(true, std::sync::atomic::Ordering::SeqCst);

    let close_future = {
      let mut state = self.close_future.lock().expect("DevEngine close future lock poisoned");
      state
        .get_or_insert_with(|| {
          let coordinator_sender = self.coordinator_sender.clone();
          let bundler = Arc::clone(&self.bundler);
          let coordinator_state = Arc::clone(&self.coordinator_state);
          (Box::pin(async move {
            Self::close_inner(coordinator_sender, bundler, coordinator_state)
              .await
              .map_err(|error| Arc::<str>::from(format!("{error:#}")))
          }) as PinBoxSendStaticFuture<DevEngineCloseResult>)
            .shared()
        })
        .clone()
    };

    match close_future.await {
      Ok(()) => Ok(()),
      Err(error) => Err(anyhow::anyhow!("{error}"))?,
    }
  }

  async fn close_inner(
    coordinator_sender: CoordinatorSender,
    bundler: Arc<Mutex<Bundler>>,
    coordinator_state: Arc<Mutex<CoordinatorState>>,
  ) -> BuildResult<()> {
    let coordinator_handle = {
      let mut coordinator_state = coordinator_state.lock().await;
      if coordinator_state.handle.is_none() {
        // `close()` before `run()` has no task to coordinate. Drop the
        // unstarted coordinator (and its watcher) and close the bundler here.
        coordinator_state.coordinator.take();
      }
      coordinator_state.handle.clone()
    };

    let Some(coordinator_handle) = coordinator_handle else {
      let mut bundler = bundler.lock().await;
      return bundler.close().await;
    };

    let (reply_sender, reply_receiver) = tokio::sync::oneshot::channel();
    let send_result = coordinator_sender
      .send(CoordinatorMsg::Close { reply: reply_sender })
      .map_err_to_unhandleable()
      .context(
        "DevEngine: failed to send Close message to coordinator - coordinator may have already terminated",
      );
    if let Err(error) = send_result {
      let coordinator_error =
        Self::merge_coordinator_task_result(error.into(), coordinator_handle.await);
      return Self::close_bundler_after_coordinator_error(bundler, coordinator_error).await;
    }

    let close_result = reply_receiver
      .await
      .map_err_to_unhandleable()
      .context("DevEngine: coordinator closed before responding to Close");
    let coordinator_task_result = coordinator_handle.await;

    match (close_result, coordinator_task_result) {
      (Ok(close_result), Ok(())) => close_result,
      (Ok(close_result), Err(task_error)) => {
        Self::merge_build_results(close_result, Err(anyhow::anyhow!("{task_error}").into()))
      }
      (Err(error), task_result) => {
        let coordinator_error = Self::merge_coordinator_task_result(error.into(), task_result);
        Self::close_bundler_after_coordinator_error(bundler, coordinator_error).await
      }
    }
  }

  fn merge_coordinator_task_result(
    coordinator_error: BatchedBuildDiagnostic,
    task_result: CoordinatorTaskResult,
  ) -> BatchedBuildDiagnostic {
    match task_result {
      Ok(()) => coordinator_error,
      Err(task_error) => {
        let mut errors = coordinator_error.into_vec();
        errors.extend(BatchedBuildDiagnostic::from(anyhow::anyhow!("{task_error}")).into_vec());
        BatchedBuildDiagnostic::new(errors)
      }
    }
  }

  fn merge_build_results(primary: BuildResult<()>, secondary: BuildResult<()>) -> BuildResult<()> {
    match (primary, secondary) {
      (Ok(()), Ok(())) => Ok(()),
      (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
      (Err(primary), Err(secondary)) => {
        let mut errors = primary.into_vec();
        errors.extend(secondary.into_vec());
        Err(BatchedBuildDiagnostic::new(errors))
      }
    }
  }

  async fn close_bundler_after_coordinator_error(
    bundler: Arc<Mutex<Bundler>>,
    coordinator_error: BatchedBuildDiagnostic,
  ) -> BuildResult<()> {
    let fallback_result = {
      let mut bundler = bundler.lock().await;
      bundler.close().await
    };

    match fallback_result {
      Ok(()) => Err(coordinator_error),
      Err(fallback_error) => {
        let mut errors = coordinator_error.into_vec();
        errors.extend(fallback_error.into_vec());
        Err(BatchedBuildDiagnostic::new(errors))
      }
    }
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
      let _ = self.coordinator_sender.send(CoordinatorMsg::WatchEvent(Ok(events)));
    }

    // Send ScheduleBuild to ensure WatchEvent is processed (FIFO),
    // and get the build future to wait on
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let _ = self.coordinator_sender.send(CoordinatorMsg::ScheduleBuildIfStale { reply: reply_tx });

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

    let (reply_sender, reply_receiver) = tokio::sync::oneshot::channel();
    self
      .coordinator_sender
      .send(CoordinatorMsg::GetWatchedFiles { reply: reply_sender })
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
    // Use special client ID "rolldown-tests" which will be recognized by HMR logic
    // to always consider modules as executed, without needing to populate the HashSet
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
