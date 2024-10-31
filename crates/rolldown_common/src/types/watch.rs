use std::{collections::HashMap, fmt::Display};

use arcstr::ArcStr;

#[allow(dead_code)]
#[derive(Default)]
pub struct WatcherEventData(Option<HashMap<&'static str, String>>);

impl WatcherEventData {
  pub fn inner(&self) -> &Option<HashMap<&'static str, String>> {
    &self.0
  }
}

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
    map.insert("id", event.path.to_string());
    map.insert("kind", event.kind.to_string());
    Self(Some(map))
  }
}

pub enum BundleEventKind {
  Start,
  BundleStart,
  BundleEnd(BundleEndEventData),
  End,
}

impl Display for BundleEventKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      BundleEventKind::Start => write!(f, "START"),
      BundleEventKind::BundleStart => write!(f, "BUNDLE_START"),
      BundleEventKind::BundleEnd(_) => write!(f, "BUNDLE_END"),
      BundleEventKind::End => write!(f, "END"),
    }
  }
}

impl From<BundleEventKind> for WatcherEventData {
  fn from(kind: BundleEventKind) -> Self {
    let mut map = HashMap::default();
    map.insert("code", kind.to_string());
    if let BundleEventKind::BundleEnd(data) = kind {
      map.insert("output", data.output);
      map.insert("duration", data.duration);
    }
    Self(Some(map))
  }
}

pub struct BundleEndEventData {
  pub output: String,
  pub duration: String,
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
