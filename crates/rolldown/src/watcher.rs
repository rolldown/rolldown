use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;

use crate::{
  watch::watcher::{wait_for_change, WatcherImpl},
  Bundler,
};

pub struct Watcher(Arc<WatcherImpl>);

impl Watcher {
  pub fn new(bundlers: Vec<Arc<Mutex<Bundler>>>) -> Result<Self> {
    let watcher = Arc::new(WatcherImpl::new(bundlers)?);

    Ok(Self(watcher))
  }

  pub async fn start(&self) {
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
