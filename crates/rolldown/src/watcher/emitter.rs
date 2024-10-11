use std::{collections::HashMap, fmt::Display, path::PathBuf, sync::Arc};

use dashmap::DashMap;

pub type SharedWatcherEmitter = Arc<WatcherEmitter>;

#[allow(dead_code)]
#[derive(Default, Clone)]
pub struct WatcherEventData(Option<Arc<HashMap<String, String>>>);

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

#[derive(PartialEq, Eq, Hash)]
pub enum WatcherEvent {
  Close,
  Event,
  ReStart,
  Change,
}

pub enum BundleEventKind {
  Start,
  BundleStart,
  BundleEnd,
  End,
}

impl Display for BundleEventKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      BundleEventKind::Start => write!(f, "START"),
      BundleEventKind::BundleStart => write!(f, "BUNDLE_START"),
      BundleEventKind::BundleEnd => write!(f, "BUNDLE_END"),
      BundleEventKind::End => write!(f, "END"),
    }
  }
}

impl From<BundleEventKind> for WatcherEventData {
  fn from(kind: BundleEventKind) -> Self {
    let mut map = HashMap::default();
    map.insert("code".to_string(), kind.to_string());
    Self(Some(Arc::new(map)))
  }
}

pub enum WatcherChangeKind {
  Create,
  Update,
  Delete,
}

impl Display for WatcherChangeKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      WatcherChangeKind::Create => write!(f, "create"),
      WatcherChangeKind::Update => write!(f, "update"),
      WatcherChangeKind::Delete => write!(f, "delete"),
    }
  }
}

pub struct WatcherChange {
  pub path: PathBuf,
  pub kind: WatcherChangeKind,
}

impl From<WatcherChange> for WatcherEventData {
  fn from(event: WatcherChange) -> Self {
    let mut map = HashMap::default();
    map.insert("id".to_string(), event.path.to_string_lossy().to_string());
    map.insert("kind".to_string(), event.kind.to_string());
    Self(Some(Arc::new(map)))
  }
}
