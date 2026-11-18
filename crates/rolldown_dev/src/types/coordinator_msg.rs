use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_fs_watcher::FsEvent;
use rolldown_utils::dashmap::FxDashSet;

use crate::type_aliases::{EnsureLatestBundleOutputSender, GetStateSender};
#[cfg(feature = "testing")]
use crate::type_aliases::{GetWatchedFilesSender, ScheduleBuildIfStaleSender};
use crate::types::error_stage::ErrorStage;

/// Messages sent to the BundleCoordinator
#[derive(Debug)]
pub enum CoordinatorMsg {
  WatchEvent(Vec<FsEvent>),
  BundleCompleted {
    /// `None` on success; on error, identifies which stage produced it
    /// so the coordinator can pick the right recovery task variant on
    /// the next file change. See `internal-docs/dev-engine/implementation.md` §7.
    error_stage: Option<ErrorStage>,
    has_generated_bundle_output: bool,
    /// The watch list the task's HMR stage added its loaded files to, when the task's
    /// rebuild then replaced it with a new list. See `BundlingTask::rebuild`.
    hmr_stage_watch_files: Option<Arc<FxDashSet<ArcStr>>>,
  },
  #[cfg(feature = "testing")]
  ScheduleBuildIfStale {
    reply: ScheduleBuildIfStaleSender,
  },
  GetState {
    reply: GetStateSender,
  },
  EnsureLatestBundleOutput {
    reply: EnsureLatestBundleOutputSender,
  },
  TriggerFullBuild,
  #[cfg(feature = "testing")]
  GetWatchedFiles {
    reply: GetWatchedFilesSender,
  },
  /// Notify that a module has changed programmatically (e.g., lazy compilation executed)
  ModuleChanged {
    module_id: String,
    /// The watch list of the build the lazy compile ran against. It holds the files
    /// the compile loaded. See `DevEngine::compile_lazy_entry`.
    watch_files: Arc<FxDashSet<ArcStr>>,
  },
  Close,
}
