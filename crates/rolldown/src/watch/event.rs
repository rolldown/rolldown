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

impl WatcherEvent {
  pub fn as_str(&self) -> &str {
    match self {
      WatcherEvent::Close => "close",
      WatcherEvent::Event(_) => "event",
      WatcherEvent::Restart => "restart",
      WatcherEvent::Change(_) => "change",
    }
  }
}

impl Display for WatcherEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
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
  Error(BundleErrorEventData),
}

impl BundleEvent {
  pub fn as_str(&self) -> &str {
    match self {
      BundleEvent::Start => "START",
      BundleEvent::BundleStart => "BUNDLE_START",
      BundleEvent::BundleEnd(_) => "BUNDLE_END",
      BundleEvent::End => "END",
      BundleEvent::Error(_) => "ERROR",
    }
  }
}

impl Display for BundleEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
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

pub struct BundleErrorEventData {
  pub error: OutputsDiagnostics,
  pub result: Arc<Mutex<Bundler>>,
}

impl Debug for BundleErrorEventData {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "BundleEndEventData {{ errors: {:?} }}", self.error)
  }
}
