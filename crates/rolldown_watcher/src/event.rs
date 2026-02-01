use arcstr::ArcStr;
use rolldown_common::WatcherChangeKind;
use std::fmt::{Debug, Display};
use std::path::PathBuf;

/// Event emitted by the watcher
#[derive(Debug, Clone)]
pub enum WatcherEvent {
  /// Watcher has closed
  Close,
  /// Bundle-related event
  Event(BundleEvent),
  /// File change detected
  Change(WatcherChangeData),
}

impl WatcherEvent {
  pub fn as_str(&self) -> &str {
    match self {
      WatcherEvent::Close => "close",
      WatcherEvent::Event(_) => "event",
      WatcherEvent::Change(_) => "change",
    }
  }
}

impl Display for WatcherEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

/// Data about a file change
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct WatcherChangeData {
  pub path: ArcStr,
  pub kind: WatcherChangeKind,
}

impl WatcherChangeData {
  pub fn new(path: ArcStr, kind: WatcherChangeKind) -> Self {
    Self { path, kind }
  }
}

/// Bundle-related events
#[derive(Debug, Clone)]
pub enum BundleEvent {
  /// Watch run is starting (all bundlers)
  Start,
  /// A single bundler is starting its build
  BundleStart { bundler_index: usize },
  /// A single bundler has finished its build
  BundleEnd(BundleEndEventData),
  /// All bundlers have finished
  End,
  /// An error occurred during bundling
  Error(BundleErrorEventData),
}

impl BundleEvent {
  pub fn as_str(&self) -> &str {
    match self {
      BundleEvent::Start => "START",
      BundleEvent::BundleStart { .. } => "BUNDLE_START",
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

/// Data for bundle end event
#[derive(Debug, Clone)]
pub struct BundleEndEventData {
  /// Index of the bundler that finished
  pub bundler_index: usize,
  /// Output path
  pub output: String,
  /// Duration of the build in milliseconds
  pub duration: u32,
}

impl BundleEndEventData {
  pub fn new(bundler_index: usize, output: String, duration: u32) -> Self {
    Self { bundler_index, output, duration }
  }
}

/// Data for bundle error event
#[derive(Debug, Clone)]
pub struct BundleErrorEventData {
  /// Index of the bundler that errored
  pub bundler_index: usize,
  /// The error messages (formatted diagnostics)
  pub errors: Vec<String>,
  /// Working directory
  pub cwd: PathBuf,
}

impl BundleErrorEventData {
  pub fn new(bundler_index: usize, errors: Vec<String>, cwd: PathBuf) -> Self {
    Self { bundler_index, errors, cwd }
  }
}
