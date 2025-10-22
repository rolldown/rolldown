use std::sync::Arc;

use anyhow::Result;
use rolldown_common::NotifyOption;
use rolldown_error::SingleBuildResult;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  watch::watcher::{WatcherImpl, wait_for_change},
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
    wait_for_change(Arc::clone(&self.0));
    self.0.start().await;
  }

  pub async fn close(&self) -> SingleBuildResult<()> {
    self.0.close().await
  }

  pub fn emitter(&self) -> Arc<crate::watch::emitter::WatcherEmitter> {
    Arc::clone(&self.0.emitter)
  }
}
