use std::sync::{
  Arc,
  atomic::{AtomicBool, AtomicU32},
};

use anyhow::Context;
use futures::{FutureExt, future::Shared};
use rolldown_common::ClientHmrUpdate;
#[cfg(feature = "testing")]
use rolldown_common::WatcherChangeKind;
use rolldown_error::{BuildResult, ResultExt};
use rolldown_fs_watcher::{FsWatcher, FsWatcherConfig, FsWatcherExt, NoopFsWatcher};
#[cfg(feature = "testing")]
use rustc_hash::FxHashSet;
use tokio::sync::{Mutex, mpsc::unbounded_channel};

use rolldown::{Bundler, BundlerBuilder, BundlerConfig, NormalizedBundlerOptions};

use crate::{
  DevOptions, SharedClients,
  bundle_coordinator::BundleCoordinator,
  dev_context::{DevContext, PinBoxSendStaticFuture},
  normalize_dev_options,
  type_aliases::CoordinatorSender,
  types::{coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot},
};

#[cfg(feature = "testing")]
use crate::ClientSession;
#[cfg(feature = "testing")]
use rolldown_utils::indexmap::FxIndexMap;
#[cfg(feature = "testing")]
use std::path::PathBuf;

pub struct CoordinatorState {
  coordinator: Option<BundleCoordinator>,
  handle: Option<Shared<PinBoxSendStaticFuture<()>>>,
}

pub struct DevEngine {
  coordinator_sender: CoordinatorSender,
  bundler: Arc<Mutex<Bundler>>,
  coordinator_state: Mutex<CoordinatorState>,
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
    };

    let event_handler = BundleCoordinator::create_watcher_event_handler(coordinator_tx.clone());

    let watcher = {
      if ctx.options.disable_watcher {
        NoopFsWatcher::with_config(event_handler, watcher_config)?.into_dyn_fs_watcher()
      } else {
        #[cfg(not(target_family = "wasm"))]
        {
          use rolldown_fs_watcher::{
            DebouncedPollFsWatcher, DebouncedRecommendedFsWatcher, PollFsWatcher,
            RecommendedFsWatcher,
          };

          match (ctx.options.use_polling, ctx.options.use_debounce) {
            // Polling + no debounce = PollFsWatcher
            (true, false) => {
              PollFsWatcher::with_config(event_handler, watcher_config)?.into_dyn_fs_watcher()
            }
            // Polling + debounce = DebouncedPollFsWatcher
            (true, true) => DebouncedPollFsWatcher::with_config(event_handler, watcher_config)?
              .into_dyn_fs_watcher(),
            // No polling + no debounce = RecommendedFsWatcher
            (false, false) => RecommendedFsWatcher::with_config(event_handler, watcher_config)?
              .into_dyn_fs_watcher(),
            // No polling + debounce = DebouncedRecommendedFsWatcher
            (false, true) => {
              DebouncedRecommendedFsWatcher::with_config(event_handler, watcher_config)?
                .into_dyn_fs_watcher()
            }
          }
        }
        #[cfg(target_family = "wasm")]
        {
          use rolldown_fs_watcher::RecommendedFsWatcher;
          // For WASM, always use NotifyWatcher (which is PollWatcher in WASM)
          // Use the FsWatcher trait implementation
          RecommendedFsWatcher::with_config(event_handler, watcher_config)?.into_dyn_fs_watcher()
        }
      }
    };

    let coordinator =
      BundleCoordinator::new(Arc::clone(&bundler), Arc::clone(&ctx), coordinator_rx, watcher);

    Ok(Self {
      coordinator_sender: coordinator_tx,
      bundler,
      coordinator_state: Mutex::new(CoordinatorState {
        coordinator: Some(coordinator),
        handle: None,
      }),
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
      let join_handle = tokio::spawn(coordinator.run());
      let coordinator_handle = Box::pin(async move {
        join_handle.await.unwrap();
      }) as PinBoxSendStaticFuture;
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
      coordinator_handle.await;
    }
    Ok(())
  }

  // Wait for ongoing bundle to finish if there is one
  pub async fn wait_for_ongoing_bundle(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    let (reply_sender, reply_receiver) = tokio::sync::oneshot::channel();
    self
      .coordinator_sender
      .send(CoordinatorMsg::GetState { reply: reply_sender })
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send GetState to coordinator")?;

    let status = reply_receiver
      .await
      .map_err_to_unhandleable()
      .context("DevEngine: coordinator closed before responding to GetStatus")?;

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
          eprintln!(
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

  pub async fn invalidate(
    &self,
    caller: String,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    self.create_error_if_closed()?;

    // Use bundler directly for invalidation (avoid message roundtrip)
    let mut updates = Vec::new();
    for client in self.clients.iter() {
      let mut bundler = self.bundler.lock().await;
      let update = bundler
        .compute_update_for_calling_invalidate(
          caller.clone(),
          first_invalidated_by.clone(),
          client.key(),
          &client.executed_modules,
          Arc::clone(&self.next_invalidate_patch_id),
        )
        .await?;
      updates.push(ClientHmrUpdate { client_id: client.key().clone(), update });
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

    // Get executed modules for this client
    let executed_modules =
      self.clients.get(&client_id).map(|c| c.executed_modules.clone()).unwrap_or_default();

    // Mark the proxy module as executed BEFORE compilation.
    // This changes the content returned by the lazy compilation plugin's load hook
    // from a stub (fetches via /lazy endpoint) to actual code that imports the real module.
    let mut bundler = self.bundler.lock().await;
    if let Some(lazy_ctx) = &bundler.lazy_compilation_context {
      lazy_ctx.mark_as_executed(&proxy_module_id);
    }

    // Compile starting from the proxy module.
    // The plugin will return new content (executed template) that imports the real module,
    // which triggers compilation of the actual module and its dependencies.
    let result = bundler
      .compile_lazy_entry(
        proxy_module_id.clone(),
        &client_id,
        &executed_modules,
        Arc::clone(&self.next_invalidate_patch_id),
      )
      .await;

    // Notify that the proxy module has changed so build output gets updated.
    // This ensures future page loads get the executed template directly.
    if result.is_ok() {
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
    if self.is_closed.swap(true, std::sync::atomic::Ordering::SeqCst) {
      return Ok(());
    }

    // Send close message to coordinator
    self.coordinator_sender.send(CoordinatorMsg::Close)
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send Close message to coordinator - coordinator may have already terminated")?;

    // Close the bundler
    let mut bundler = self.bundler.lock().await;
    bundler.close().await?;

    // Wait for coordinator to close (coordinator handles watcher cleanup)
    let coordinator_state = self.coordinator_state.lock().await;
    if let Some(coordinator_handle) = coordinator_state.handle.clone() {
      coordinator_handle.await;
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
    for (path, event) in changed_files {
      // Create a synthetic file change event to simulate real file system changes
      let notify_event = notify::Event {
        kind: if event == WatcherChangeKind::Delete {
          notify::EventKind::Remove(notify::event::RemoveKind::Any)
        } else {
          notify::EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Any))
        },
        paths: vec![path],
        attrs: notify::event::EventAttributes::default(),
      };

      let event =
        rolldown_fs_watcher::FsEvent { detail: notify_event, time: std::time::Instant::now() };

      // Send WatchEvent message to coordinator (simulates real file change)
      // The coordinator will automatically schedule a build via handle_file_changes
      let _ = self.coordinator_sender.send(CoordinatorMsg::WatchEvent(Ok(vec![event])));
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
  pub fn create_client_for_testing(&self) {
    let client_session = ClientSession::default();
    // Use special client ID "rolldown-tests" which will be recognized by HMR logic
    // to always consider modules as executed, without needing to populate the HashSet
    self.clients.insert("rolldown-tests".to_string(), client_session);
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
  pub last_full_build_failed: bool,
  pub has_stale_output: bool,
}

impl From<CoordinatorStateSnapshot> for BundleState {
  fn from(snapshot: CoordinatorStateSnapshot) -> Self {
    Self {
      last_full_build_failed: snapshot.last_full_build_failed,
      has_stale_output: snapshot.has_stale_output,
    }
  }
}
