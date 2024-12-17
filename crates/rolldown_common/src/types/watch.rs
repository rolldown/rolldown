use std::fmt::Display;

use arcstr::ArcStr;

use crate::OutputsDiagnostics;

#[derive(Debug)]
pub enum WatcherEvent {
  Close,
  Event(BundleEvent),
  ReStart,
  Change(WatcherChangeData),
}

impl Display for WatcherEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      WatcherEvent::Close => write!(f, "close"),
      WatcherEvent::Event(_) => write!(f, "event"),
      WatcherEvent::ReStart => write!(f, "restart"),
      WatcherEvent::Change(_) => write!(f, "change"),
    }
  }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct WatcherChangeData {
  pub path: ArcStr,
  pub kind: WatcherChangeKind,
}

#[derive(Debug)]
pub enum BundleEvent {
  Start,
  BundleStart,
  BundleEnd(BundleEndEventData),
  End,
  Error(OutputsDiagnostics),
}

impl Display for BundleEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      BundleEvent::Start => write!(f, "START"),
      BundleEvent::BundleStart => write!(f, "BUNDLE_START"),
      BundleEvent::BundleEnd(_) => write!(f, "BUNDLE_END"),
      BundleEvent::End => write!(f, "END"),
      BundleEvent::Error(_) => write!(f, "ERROR"),
    }
  }
}

#[derive(Debug)]
pub struct BundleEndEventData {
  pub output: String,
  pub duration: u32,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
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
