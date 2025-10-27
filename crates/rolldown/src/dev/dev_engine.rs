use std::{
  collections::VecDeque,
  ops::Deref,
  path::PathBuf,
  sync::{Arc, atomic::AtomicBool},
};

use anyhow::Context;
use arcstr::ArcStr;
use futures::{FutureExt, future::Shared};
use rolldown_common::ClientHmrUpdate;
use rolldown_error::{BuildResult, ResultExt};
use rolldown_utils::{dashmap::FxDashSet, indexmap::FxIndexSet};
use rolldown_watcher::{DynWatcher, NoopWatcher, Watcher, WatcherConfig, WatcherExt};
use sugar_path::SugarPath;
use tokio::sync::{Mutex, mpsc::unbounded_channel};

use crate::{
  Bundler, BundlerBuilder,
  dev::{
    SharedClients,
    build_driver::{BuildDriver, SharedBuildDriver},
    build_driver_service::{BuildDriverService, BuildMessage},
    build_state_machine::BuildStateMachine,
    dev_context::{DevContext, PinBoxSendStaticFuture, SharedDevContext},
    dev_options::{DevOptions, normalize_dev_options},
    types::{client_session::ClientSession, task_input::TaskInput},
  },
};

pub struct BuildDriverServiceState {
  service: Option<BuildDriverService>,
  handle: Option<Shared<PinBoxSendStaticFuture<()>>>,
}

pub struct DevEngine {
  build_driver: SharedBuildDriver,
  watcher: Mutex<DynWatcher>,
  watched_files: FxDashSet<ArcStr>,
  build_driver_service_state: Mutex<BuildDriverServiceState>,
  ctx: SharedDevContext,
  pub clients: SharedClients,
  is_closed: AtomicBool,
}

impl DevEngine {
  pub fn new(bundler_builder: BundlerBuilder, options: DevOptions) -> BuildResult<Self> {
    Self::with_bundler(Arc::new(Mutex::new(bundler_builder.build()?)), options)
  }

  pub fn with_bundler(bundler: Arc<Mutex<Bundler>>, options: DevOptions) -> BuildResult<Self> {
    let normalized_options = normalize_dev_options(options);

    let (build_channel_tx, build_channel_rx) = unbounded_channel::<BuildMessage>();

    let clients = SharedClients::default();

    let ctx = Arc::new(DevContext {
      state: Mutex::new(BuildStateMachine {
        queued_tasks: VecDeque::from([TaskInput::new_initial_build_task()]),
        ..BuildStateMachine::new()
      }),
      options: normalized_options,
      build_channel_tx,
      clients: Arc::clone(&clients),
    });
    let build_driver = Arc::new(BuildDriver::new(bundler, Arc::clone(&ctx)));

    let build_driver_service =
      BuildDriverService::new(Arc::clone(&build_driver), Arc::clone(&ctx), build_channel_rx);
    let watcher_config = WatcherConfig {
      poll_interval: ctx.options.poll_interval,
      debounce_delay: ctx.options.debounce_duration,
      compare_contents_for_polling: ctx.options.compare_contents_for_polling,
      debounce_tick_rate: ctx.options.debounce_tick_rate,
    };

    let watcher = {
      if ctx.options.disable_watcher {
        NoopWatcher::with_config(
          build_driver_service.create_watcher_event_handler(),
          watcher_config,
        )?
        .into_dyn_watcher()
      } else {
        #[cfg(not(target_family = "wasm"))]
        {
          use rolldown_watcher::{
            DebouncedPollWatcher, DebouncedRecommendedWatcher, PollWatcher, RecommendedWatcher,
          };

          match (ctx.options.use_polling, ctx.options.use_debounce) {
            // Polling + no debounce = PollWatcher
            (true, false) => PollWatcher::with_config(
              build_driver_service.create_watcher_event_handler(),
              watcher_config,
            )?
            .into_dyn_watcher(),
            // Polling + debounce = DebouncedPollWatcher
            (true, true) => DebouncedPollWatcher::with_config(
              build_driver_service.create_watcher_event_handler(),
              watcher_config,
            )?
            .into_dyn_watcher(),
            // No polling + no debounce = RecommendedWatcher
            (false, false) => RecommendedWatcher::with_config(
              build_driver_service.create_watcher_event_handler(),
              watcher_config,
            )?
            .into_dyn_watcher(),
            // No polling + debounce = DebouncedRecommendedWatcher
            (false, true) => DebouncedRecommendedWatcher::with_config(
              build_driver_service.create_watcher_event_handler(),
              watcher_config,
            )?
            .into_dyn_watcher(),
          }
        }
        #[cfg(target_family = "wasm")]
        {
          use rolldown_watcher::RecommendedWatcher;
          // For WASM, always use NotifyWatcher (which is PollWatcher in WASM)
          // Use the Watcher trait implementation
          RecommendedWatcher::with_config(
            build_driver_service.create_watcher_event_handler(),
            watcher_config,
          )?
          .into_dyn_watcher()
        }
      }
    };

    Ok(Self {
      build_driver,
      watcher: Mutex::new(watcher),
      watched_files: FxDashSet::default(),
      build_driver_service_state: Mutex::new(BuildDriverServiceState {
        service: Some(build_driver_service),
        handle: None,
      }),
      ctx,
      clients,
      is_closed: AtomicBool::new(false),
    })
  }

  pub async fn run(&self) -> BuildResult<()> {
    let mut build_service_state = self.build_driver_service_state.lock().await;

    if build_service_state.service.is_none() {
      // The watcher service is already running.
      return Ok(());
    }

    self.build_driver.ensure_latest_build_output().await.expect("FIXME: Should not fail");

    if let Some(watcher_service) = build_service_state.service.take() {
      let join_handle = tokio::spawn(watcher_service.run());
      let watcher_service_handle = Box::pin(async move {
        join_handle.await.unwrap();
      }) as PinBoxSendStaticFuture;
      build_service_state.handle = Some(watcher_service_handle.shared());
    }
    drop(build_service_state);

    let bundler = self.build_driver.bundler.lock().await;
    // hyf0 TODO: `get_watch_files` is not a proper API to tell which files should be watched.
    let watch_files = bundler.get_watch_files();

    let mut watcher = self.watcher.lock().await;
    let mut paths_mut = watcher.paths_mut();
    for watch_file in watch_files.iter() {
      let watch_file = &**watch_file;
      tracing::trace!("watch file: {:?}", watch_file);
      if !self.watched_files.contains(watch_file) {
        self.watched_files.insert(watch_file.to_string().into());
        paths_mut.add(watch_file.as_path(), notify::RecursiveMode::NonRecursive)?;
      }
    }
    paths_mut.commit()?;
    Ok(())
  }

  pub async fn wait_for_build_driver_service_close(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;
    let service_state = self.build_driver_service_state.lock().await;
    if let Some(service_handle) = service_state.handle.clone() {
      service_handle.await;
    }
    Ok(())
  }

  pub async fn ensure_current_build_finish(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;
    self.ctx.ensure_current_build_finish().await;
    Ok(())
  }

  pub async fn invalidate(
    &self,
    caller: String,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    self.create_error_if_closed()?;
    self.build_driver.invalidate(caller, first_invalidated_by).await
  }

  pub async fn close(&self) -> BuildResult<()> {
    if self.is_closed.swap(true, std::sync::atomic::Ordering::SeqCst) {
      return Ok(());
    }

    // Send close message to build driver service
    self.ctx.build_channel_tx.send(BuildMessage::Close)
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send Close message to build service - service may have already terminated")?;

    // Clean up watcher
    let watcher =
      std::mem::replace(&mut *self.watcher.lock().await, NoopWatcher.into_dyn_watcher());
    std::mem::drop(watcher);

    // Close the bundler
    let mut bundler = self.build_driver.bundler.lock().await;
    bundler.close().await?;

    // Wait for build driver service to close
    let service_state = self.build_driver_service_state.lock().await;
    if let Some(service_handle) = service_state.handle.clone() {
      service_handle.await;
    }
    Ok(())
  }

  /// For testing purpose.
  pub async fn ensure_task_with_changed_files(&self, changed_files: FxIndexSet<PathBuf>) {
    self.build_driver.handle_file_changes(changed_files).await;
    if let Some(status) = self.build_driver.schedule_build_if_stale().await.unwrap() {
      status.0.await;
    }
  }

  pub fn create_client_for_testing(&self) {
    let client_session = ClientSession::default();
    // Use special client ID "rolldown-tests" which will be recognized by HMR logic
    // to always consider modules as executed, without needing to populate the HashSet
    self.clients.insert("rolldown-tests".to_string(), client_session);
  }

  pub fn is_closed(&self) -> bool {
    self.is_closed.load(std::sync::atomic::Ordering::SeqCst)
  }

  fn create_error_if_closed(&self) -> BuildResult<()> {
    if self.is_closed.load(std::sync::atomic::Ordering::SeqCst) {
      Err(anyhow::anyhow!("Dev engine is closed"))?;
    }
    Ok(())
  }
}

impl Deref for DevEngine {
  type Target = BuildDriver;

  fn deref(&self) -> &Self::Target {
    &self.build_driver
  }
}
