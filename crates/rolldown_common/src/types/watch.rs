use std::fmt::Display;

use arcstr::ArcStr;

pub enum WatcherEventData {
  Empty,
  WatcherChange(WatcherChangeData),
  BundleEvent(BundleEventKind),
}

impl Default for WatcherEventData {
  fn default() -> Self {
    Self::Empty
  }
}

#[derive(PartialEq, Eq, Hash)]
pub enum WatcherEvent {
  Close,
  Event,
  ReStart,
  Change,
}

pub struct WatcherChangeData {
  pub path: ArcStr,
  pub kind: WatcherChangeKind,
}

impl From<WatcherChangeData> for WatcherEventData {
  fn from(event: WatcherChangeData) -> Self {
    WatcherEventData::WatcherChange(event)
  }
}

pub enum BundleEventKind {
  Start,
  BundleStart,
  BundleEnd(BundleEndEventData),
  End,
  Error(String),
}

impl Display for BundleEventKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      BundleEventKind::Start => write!(f, "START"),
      BundleEventKind::BundleStart => write!(f, "BUNDLE_START"),
      BundleEventKind::BundleEnd(_) => write!(f, "BUNDLE_END"),
      BundleEventKind::End => write!(f, "END"),
      BundleEventKind::Error(_) => write!(f, "ERROR"),
    }
  }
}

impl From<BundleEventKind> for WatcherEventData {
  fn from(kind: BundleEventKind) -> Self {
    WatcherEventData::BundleEvent(kind)
  }
}

pub struct BundleEndEventData {
  pub output: String,
  pub duration: u32,
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
