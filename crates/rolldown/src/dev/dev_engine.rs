use std::sync::Arc;

use arcstr::ArcStr;
use futures::{FutureExt, future::Shared};
use rolldown_error::BuildResult;
use rolldown_utils::dashmap::FxDashSet;
use rolldown_watcher::Watcher;
use sugar_path::SugarPath;
use tokio::sync::Mutex;

use crate::{
  BundlerBuilder,
  dev::{
    build_driver::{BuildDriver, SharedBuildDriver},
    dev_context::PinBoxSendStaticFuture,
    watcher_event_service::WatcherEventService,
  },
};

pub struct DevEngine<W: Watcher + Send + 'static> {
  build_driver: SharedBuildDriver,
  watcher: Mutex<W>,
  watched_files: FxDashSet<ArcStr>,
  watcher_service: Option<WatcherEventService>,
  watcher_service_handle: Option<Shared<PinBoxSendStaticFuture<()>>>,
}

impl<W: Watcher + Send + 'static> DevEngine<W> {
  pub fn new(bundler_builder: BundlerBuilder) -> BuildResult<Self> {
    let build_driver = Arc::new(BuildDriver::new(bundler_builder));

    let watcher_event_service = WatcherEventService::new(Arc::clone(&build_driver));
    let watcher = W::new(watcher_event_service.create_event_handler())?;

    Ok(Self {
      build_driver,
      watcher: Mutex::new(watcher),
      watched_files: FxDashSet::default(),
      watcher_service: Some(watcher_event_service),
      watcher_service_handle: None,
    })
  }

  pub async fn run(&mut self) {
    if let Some(build_process_future) = self.build_driver.schedule_build(vec![]).await {
      build_process_future.await;
    } else {
      self.build_driver.wait_for_current_build_finish().await;
    }

    if let Some(watcher_service) = self.watcher_service.take() {
      let watcher_service_handle = tokio::spawn(watcher_service.run());
      let watcher_service_handle = Box::pin(async move {
        watcher_service_handle.await.expect("How we handle this error?");
      }) as PinBoxSendStaticFuture;
      self.watcher_service_handle = Some(watcher_service_handle.shared());
    }

    let bundler = self.build_driver.bundler.lock().await;
    // hyf0 TODO: `get_watch_files` is not a proper API to tell which files should be watched.
    let watch_files = bundler.get_watch_files();

    let mut watcher = self.watcher.lock().await;
    // let mut watched_paths = watcher.paths_mut();
    for watch_file in watch_files.iter() {
      let watch_file = &*watch_file;
      if self.watched_files.contains(watch_file) {
        continue;
      }
      self.watched_files.insert(watch_file.clone());
      watcher.watch(watch_file.as_path(), notify::RecursiveMode::NonRecursive).unwrap();
    }
  }

  pub async fn wait_for_close(&self) {
    if let Some(watcher_service_handle) = self.watcher_service_handle.clone() {
      watcher_service_handle.await;
    }
  }
}
