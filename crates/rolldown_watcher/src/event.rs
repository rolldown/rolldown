use crate::watch_task::WatchTaskIdx;
use rolldown::Bundler;
use rolldown_error::BuildDiagnostic;
use std::fmt::{Debug, Display};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Watch-related events
#[derive(Debug, Clone)]
pub enum WatchEvent {
  /// Watch run is starting (all tasks)
  Start,
  /// A single bundle is starting its build
  BundleStart(BundleStartEventData),
  /// A single bundle has finished its build
  BundleEnd(BundleEndEventData),
  /// All tasks have finished
  End,
  /// An error occurred during bundling
  Error(WatchErrorEventData),
}

impl WatchEvent {
  pub fn as_str(&self) -> &str {
    match self {
      WatchEvent::Start => "START",
      WatchEvent::BundleStart(_) => "BUNDLE_START",
      WatchEvent::BundleEnd(_) => "BUNDLE_END",
      WatchEvent::End => "END",
      WatchEvent::Error(_) => "ERROR",
    }
  }
}

impl Display for WatchEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

/// Data for bundle start event
#[derive(Debug, Clone)]
pub struct BundleStartEventData {
  pub task_index: WatchTaskIdx,
}

/// Data for bundle end event
#[derive(Clone)]
pub struct BundleEndEventData {
  pub task_index: WatchTaskIdx,
  pub output: String,
  pub duration: u32,
  pub bundler: Arc<Mutex<Bundler>>,
}

impl Debug for BundleEndEventData {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BundleEndEventData")
      .field("task_index", &self.task_index)
      .field("output", &self.output)
      .field("duration", &self.duration)
      .finish_non_exhaustive()
  }
}

/// Data for task error event
#[derive(Clone)]
pub struct WatchErrorEventData {
  pub task_index: WatchTaskIdx,
  /// Raw diagnostics preserved for rich error conversion at the binding layer.
  /// Wrapped in `Arc` because `BuildDiagnostic` is not `Clone`.
  pub diagnostics: Arc<[BuildDiagnostic]>,
  pub cwd: PathBuf,
  pub bundler: Arc<Mutex<Bundler>>,
}

impl Debug for WatchErrorEventData {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("WatchErrorEventData")
      .field("task_index", &self.task_index)
      .field("diagnostics", &self.diagnostics)
      .field("cwd", &self.cwd)
      .finish_non_exhaustive()
  }
}
