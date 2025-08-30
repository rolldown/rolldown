use std::{ops::Deref, sync::Arc};

use arcstr::ArcStr;
use futures::{FutureExt, future::Shared};
use rolldown_error::BuildResult;
use rolldown_utils::dashmap::FxDashSet;
use rolldown_watcher::Watcher;
use sugar_path::SugarPath;
use tokio::sync::Mutex;

use crate::{
  Bundler, BundlerBuilder,
  dev::{
    build_driver::{BuildDriver, SharedBuildDriver},
    build_state_machine::BuildStateMachine,
    dev_context::{DevContext, PinBoxSendStaticFuture, SharedDevContext},
    dev_options::{DevOptions, normalize_dev_options},
    watcher_event_service::WatcherEventService,
  },
};

pub struct WatchServiceState {
  service: Option<WatcherEventService>,
  handle: Option<Shared<PinBoxSendStaticFuture<()>>>,
}

pub struct DevEngine<W> {
  build_driver: SharedBuildDriver,
  watcher: Mutex<W>,
  watched_files: FxDashSet<ArcStr>,
  watch_service_state: Mutex<WatchServiceState>,
  ctx: SharedDevContext,
}

impl<W: Watcher + Send + 'static> DevEngine<W> {
  pub fn new(bundler_builder: BundlerBuilder, options: DevOptions) -> BuildResult<Self> {
    Self::with_bundler(Arc::new(Mutex::new(bundler_builder.build())), options)
  }

  pub fn with_bundler(bundler: Arc<Mutex<Bundler>>, options: DevOptions) -> BuildResult<Self> {
    let normalized_options = normalize_dev_options(options);

    let ctx = Arc::new(DevContext {
      state: Mutex::new(BuildStateMachine::new()),
      options: normalized_options,
    });
    let build_driver = Arc::new(BuildDriver::new(bundler, Arc::clone(&ctx)));

    let watcher_event_service =
      WatcherEventService::new(Arc::clone(&build_driver), Arc::clone(&ctx));
    let watcher = W::new(watcher_event_service.create_event_handler())?;

    Ok(Self {
      build_driver,
      watcher: Mutex::new(watcher),
      watched_files: FxDashSet::default(),
      watch_service_state: Mutex::new(WatchServiceState {
        service: Some(watcher_event_service),
        handle: None,
      }),
      ctx,
    })
  }

  pub async fn run(&self) {
    let mut watch_service_state = self.watch_service_state.lock().await;

    if watch_service_state.service.is_none() {
      // The watcher service is already running.
      return;
    }

    self.build_driver.ensure_latest_build().await.expect("FIXME: Should not fail");

    if let Some(watcher_service) = watch_service_state.service.take() {
      let join_handle = tokio::spawn(watcher_service.run());
      let watcher_service_handle = Box::pin(async move {
        join_handle.await.unwrap();
      }) as PinBoxSendStaticFuture;
      watch_service_state.handle = Some(watcher_service_handle.shared());
    }
    drop(watch_service_state);

    let bundler = self.build_driver.bundler.lock().await;
    // hyf0 TODO: `get_watch_files` is not a proper API to tell which files should be watched.
    let watch_files = bundler.get_watch_files();

    let mut watcher = self.watcher.lock().await;
    // let mut watched_paths = watcher.paths_mut();
    for watch_file in watch_files.iter() {
      let watch_file = &*watch_file;
      // FIXME: invalid file should be filtered by rolldown. This is a workaround.
      if !watch_file.as_path().is_absolute() {
        continue;
      }
      tracing::trace!("watch file: {:?}", watch_file);
      if self.watched_files.contains(watch_file) {
        continue;
      }
      self.watched_files.insert(watch_file.clone());
      watcher.watch(watch_file.as_path(), notify::RecursiveMode::NonRecursive).unwrap();
    }
  }

  pub async fn wait_for_close(&self) {
    let watch_service_state = self.watch_service_state.lock().await;
    if let Some(watcher_service_handle) = watch_service_state.handle.clone() {
      watcher_service_handle.await;
    }
  }

  pub async fn ensure_current_build_finish(&self) {
    self.ctx.ensure_current_build_finish().await;
  }
}

impl<T> Deref for DevEngine<T> {
  type Target = BuildDriver;

  fn deref(&self) -> &Self::Target {
    &self.build_driver
  }
}
