use std::sync::{
  Arc,
  atomic::{AtomicBool, AtomicU32},
};

use anyhow::Context;
use futures::{FutureExt, future::Shared};
use rolldown_common::ClientHmrUpdate;
use rolldown_error::{BuildResult, ResultExt};
use rolldown_fs_watcher::{FsWatcher, FsWatcherConfig, FsWatcherExt, NoopFsWatcher};
use tokio::sync::{Mutex, mpsc::unbounded_channel};

use crate::{
  Bundler, BundlerBuilder,
  dev::{
    DevOptions, SharedClients,
    bundle_coordinator::BundleCoordinator,
    dev_context::{DevContext, PinBoxSendStaticFuture},
    normalize_dev_options,
    type_aliases::CoordinatorSender,
    types::coordinator_msg::CoordinatorMsg,
  },
};

#[cfg(feature = "testing")]
use crate::dev::ClientSession;
#[cfg(feature = "testing")]
use rolldown_utils::indexmap::FxIndexSet;
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
  pub fn new(bundler_builder: BundlerBuilder, options: DevOptions) -> BuildResult<Self> {
    Self::with_bundler(Arc::new(Mutex::new(bundler_builder.build()?)), options)
  }

  pub fn with_bundler(bundler: Arc<Mutex<Bundler>>, options: DevOptions) -> BuildResult<Self> {
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
      .send(CoordinatorMsg::GetStatus { reply: reply_sender })
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send GetStatus to coordinator")?;

    let status = reply_receiver
      .await
      .map_err_to_unhandleable()
      .context("DevEngine: coordinator closed before responding to GetStatus")?;

    if let Some(bundling_future) = status.running_future {
      bundling_future.await;
    }

    Ok(())
  }

  pub async fn has_latest_bundle_output(&self) -> BuildResult<bool> {
    self.create_error_if_closed()?;

    let (reply_sender, reply_receiver) = tokio::sync::oneshot::channel();
    self
      .coordinator_sender
      .send(CoordinatorMsg::GetStatus { reply: reply_sender })
      .map_err_to_unhandleable()
      .context(
        "DevEngine: failed to send GetStatus to coordinator within has_latest_bundle_output",
      )?;

    let status = reply_receiver
      .await
      .map_err_to_unhandleable()
      .context("DevEngine: coordinator closed before responding to GetStatus within has_latest_bundle_output")?;

    Ok(!status.has_stale_output)
  }

  // Ensure there's latest bundle output available for browser loading/reloading scenarios
  pub async fn ensure_latest_bundle_output(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    let mut count = 0;

    loop {
      count += 1;
      if count > 1000 {
        eprintln!(
          "Debug: `ensure_latest_bundle_output` wait for 1000 times build, something might be wrong"
        );
        break;
      }

      // Get current build status from coordinator
      let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
      self
        .coordinator_sender
        .send(CoordinatorMsg::GetStatus { reply: reply_tx })
        .map_err_to_unhandleable()
        .context("DevEngine: failed to send GetStatus to coordinator")?;
      let status = reply_rx
        .await
        .map_err_to_unhandleable()
        .context("DevEngine: coordinator closed before responding to GetStatus")?;

      if let Some(building_future) = status.running_future {
        tracing::trace!("Waiting for current build to finish...; {:?}", status.initial_build_state);
        building_future.await;
      } else {
        tracing::trace!("No current build in progress...");
        if status.has_stale_output {
          // Need to schedule a build
          let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
          self
            .coordinator_sender
            .send(CoordinatorMsg::ScheduleBuild { reply: reply_tx })
            .map_err_to_unhandleable()
            .context("DevEngine: failed to send ScheduleBuild to coordinator")?;

          let schedule_result = reply_rx
            .await
            .map_err_to_unhandleable()
            .context("DevEngine: coordinator closed before responding to ScheduleBuild")??;

          if let Some((building_future, _)) = schedule_result {
            building_future.await;
          } else {
            // No build was scheduled, which means there's no task in queue
            // Queue the appropriate task based on initial build state
            // This shouldn't normally happen as coordinator queues initial task
            break;
          }
        } else {
          // Build output is fresh, we're done
          break;
        }
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

  #[cfg(feature = "testing")]
  pub async fn ensure_task_with_changed_files(&self, changed_files: FxIndexSet<PathBuf>) {
    // Create a synthetic file change event to simulate real file system changes
    let notify_event = notify::Event {
      kind: notify::EventKind::Modify(notify::event::ModifyKind::Data(
        notify::event::DataChange::Any,
      )),
      paths: changed_files.into_iter().collect(),
      attrs: notify::event::EventAttributes::default(),
    };

    let event =
      rolldown_fs_watcher::FsEvent { detail: notify_event, time: std::time::Instant::now() };

    // Send WatchEvent message to coordinator (simulates real file change)
    // The coordinator will automatically schedule a build via handle_file_changes
    let _ = self.coordinator_sender.send(CoordinatorMsg::WatchEvent(Ok(vec![event])));

    // Send ScheduleBuild to ensure WatchEvent is processed (FIFO),
    // and get the build future to wait on
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let _ = self.coordinator_sender.send(CoordinatorMsg::ScheduleBuild { reply: reply_tx });

    // Wait for the build that was triggered by the file change
    if let Ok(Ok(Some((future, _)))) = reply_rx.await {
      future.await;
    }
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
