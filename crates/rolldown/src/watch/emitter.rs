use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::watch::event::WatcherEvent;

// Shared reference type for watcher emitter across async tasks
pub type SharedWatcherEmitter = Arc<WatcherEmitter>;

pub struct WatcherEmitter {
  // Shared sender for multiple producers to emit watcher events
  tx: Arc<std::sync::mpsc::Sender<WatcherEvent>>,
  // Shared async-safe receiver for consuming watcher events
  pub rx: Arc<Mutex<std::sync::mpsc::Receiver<WatcherEvent>>>,
}

impl WatcherEmitter {
  pub fn new() -> Self {
    let (tx, rx) = std::sync::mpsc::channel::<WatcherEvent>();
    // Wrap channel endpoints in Arc for sharing across async tasks
    Self { tx: Arc::new(tx), rx: Arc::new(Mutex::new(rx)) }
  }

  #[allow(clippy::needless_pass_by_value)]
  pub fn emit(&self, event: WatcherEvent) -> Result<()> {
    self.tx.send(event)?;
    Ok(())
  }
}
