use std::sync::Arc;

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
    dev_context::PinBoxSendStaticFuture,
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
}

impl<W: Watcher + Send + 'static> DevEngine<W> {
  pub fn new(bundler_builder: BundlerBuilder) -> BuildResult<Self> {
    Self::with_bundler(Arc::new(Mutex::new(bundler_builder.build())))
  }

  pub fn with_bundler(bundler: Arc<Mutex<Bundler>>) -> BuildResult<Self> {
    let build_driver = Arc::new(BuildDriver::new(bundler));

    let watcher_event_service = WatcherEventService::new(Arc::clone(&build_driver));
    let watcher = W::new(watcher_event_service.create_event_handler())?;

    Ok(Self {
      build_driver,
      watcher: Mutex::new(watcher),
      watched_files: FxDashSet::default(),
      watch_service_state: Mutex::new(WatchServiceState {
        service: Some(watcher_event_service),
        handle: None,
      }),
    })
  }

  pub async fn run(&self) {
    let mut watch_service_state = self.watch_service_state.lock().await;

    if watch_service_state.service.is_none() {
      // The watcher service is already running.
      return;
    }

    if let Some(build_process_future) = self.build_driver.schedule_build(vec![]).await {
      build_process_future.await;
    } else {
      self.build_driver.ensure_current_build_finish().await;
    }

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
    self.build_driver.ensure_current_build_finish().await;
  }
}

impl<W> Drop for DevEngine<W> {
  fn drop(&mut self) {
    tracing::trace!("DevEngine dropped");
  }
}
