use std::sync::Arc;

use dashmap::DashMap;
use rolldown_common::{WatcherEvent, WatcherEventData};

pub type SharedWatcherEmitter = Arc<WatcherEmitter>;

pub type Listener = Box<dyn Fn(WatcherEventData) + Send + Sync>;

pub struct WatcherEmitter {
  closed: bool,
  listeners: DashMap<WatcherEvent, Vec<Listener>>,
}

impl WatcherEmitter {
  pub fn new() -> Self {
    Self { closed: false, listeners: DashMap::default() }
  }

  pub fn close(&mut self) {
    if self.closed {
      return;
    }
    self.closed = true;
    self.emit(WatcherEvent::Close, WatcherEventData::default());
  }

  #[allow(clippy::needless_pass_by_value)]
  pub fn emit(&self, event: WatcherEvent, data: WatcherEventData) {
    if let Some(listeners) = self.listeners.get(&event) {
      for listener in listeners.iter() {
        listener(data.clone());
      }
    }
  }

  pub fn on(&self, event: WatcherEvent, listener: Listener) {
    self.listeners.entry(event).or_default().push(Box::new(listener));
  }
}
