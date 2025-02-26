use std::sync::Arc;

use anyhow::Result;
use rolldown_common::NotifyOption;
use tokio::sync::Mutex;

use crate::{
  watch::watcher::{wait_for_change, wait_for_invalidate_run, WatcherImpl},
  Bundler,
};

pub struct Watcher(Arc<WatcherImpl>);

impl Watcher {
  pub fn new(
    bundlers: Vec<Arc<Mutex<Bundler>>>,
    notify_option: Option<NotifyOption>,
  ) -> Result<Self> {
    let watcher = Arc::new(WatcherImpl::new(bundlers, notify_option)?);

    Ok(Self(watcher))
  }

  pub async fn start(&self) {
    wait_for_invalidate_run(Arc::clone(&self.0));
    wait_for_change(Arc::clone(&self.0));
    self.0.start().await;
  }

  pub async fn close(&self) -> Result<()> {
    self.0.close().await
  }

  pub fn emitter(&self) -> Arc<crate::watch::emitter::WatcherEmitter> {
    Arc::clone(&self.0.emitter)
  }
}
