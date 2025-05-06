use std::{
  fmt::{Debug, Display},
  sync::Arc,
};

use arcstr::ArcStr;

use rolldown_common::{OutputsDiagnostics, WatcherChangeKind};
use tokio::sync::Mutex;

use crate::Bundler;

#[derive(Debug)]
pub enum WatcherEvent {
  Close,
  Event(BundleEvent),
  Restart,
  Change(WatcherChangeData),
}

impl Display for WatcherEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      WatcherEvent::Close => write!(f, "close"),
      WatcherEvent::Event(_) => write!(f, "event"),
      WatcherEvent::Restart => write!(f, "restart"),
      WatcherEvent::Change(_) => write!(f, "change"),
    }
  }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
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

pub struct BundleEndEventData {
  pub output: String,
  pub duration: u32,
  pub result: Arc<Mutex<Bundler>>,
}

impl Debug for BundleEndEventData {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "BundleEndEventData {{ output: {}, duration: {} }}", self.output, self.duration)
  }
}
