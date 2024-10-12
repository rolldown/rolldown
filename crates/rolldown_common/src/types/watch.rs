use std::{collections::HashMap, fmt::Display, sync::Arc};

use arcstr::ArcStr;

#[allow(dead_code)]
#[derive(Default, Clone)]
pub struct WatcherEventData(Option<Arc<HashMap<String, String>>>);

#[derive(PartialEq, Eq, Hash)]
pub enum WatcherEvent {
  Close,
  Event,
  ReStart,
  Change,
}

pub struct WatcherChange {
  pub path: ArcStr,
  pub kind: WatcherChangeKind,
}

impl From<WatcherChange> for WatcherEventData {
  fn from(event: WatcherChange) -> Self {
    let mut map = HashMap::default();
    map.insert("id".to_string(), event.path.to_string());
    map.insert("kind".to_string(), event.kind.to_string());
    Self(Some(Arc::new(map)))
  }
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

#[derive(Copy, Clone)]
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
