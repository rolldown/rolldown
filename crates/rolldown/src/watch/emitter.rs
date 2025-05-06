use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::watch::event::WatcherEvent;

pub type SharedWatcherEmitter = Arc<WatcherEmitter>;

pub struct WatcherEmitter {
  tx: Arc<std::sync::mpsc::Sender<WatcherEvent>>,
  pub rx: Arc<Mutex<std::sync::mpsc::Receiver<WatcherEvent>>>,
}

impl WatcherEmitter {
  pub fn new() -> Self {
    let (tx, rx) = std::sync::mpsc::channel::<WatcherEvent>();
    Self { tx: Arc::new(tx), rx: Arc::new(Mutex::new(rx)) }
  }

  #[allow(clippy::needless_pass_by_value)]
  pub fn emit(&self, event: WatcherEvent) -> Result<()> {
    self.tx.send(event)?;
    Ok(())
  }
}
