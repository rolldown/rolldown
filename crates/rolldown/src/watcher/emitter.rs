use anyhow::Result;
use std::{future::Future, pin::Pin, sync::Arc};

use dashmap::DashMap;
use rolldown_common::{WatcherEvent, WatcherEventData};

pub type SharedWatcherEmitter = Arc<WatcherEmitter>;

pub type Listener = Box<
  dyn Fn(&WatcherEventData) -> Pin<Box<(dyn Future<Output = anyhow::Result<()>> + Send + 'static)>>
    + Send
    + Sync,
>;

pub struct WatcherEmitter {
  listeners: DashMap<WatcherEvent, Vec<Listener>>,
}

impl WatcherEmitter {
  pub fn new() -> Self {
    Self { listeners: DashMap::default() }
  }

  #[allow(clippy::needless_pass_by_value)]
  pub async fn emit(&self, event: WatcherEvent, data: WatcherEventData) -> Result<()> {
    if let Some(listeners) = self.listeners.get(&event) {
      for listener in listeners.iter() {
        listener(&data).await?;
      }
    }
    Ok(())
  }

  #[allow(dead_code)]
  pub fn on(&self, event: WatcherEvent, listener: Listener) {
    self.listeners.entry(event).or_default().push(Box::new(listener));
  }
}
