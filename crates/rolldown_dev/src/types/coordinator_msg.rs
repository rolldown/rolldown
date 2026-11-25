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
  /// A pull with lazy routes merged a scan into the graph, or found a lost HMR update. See
  /// `DevEngine::render_patch`.
  LazyCompiled {
    /// The watch list of the build the scan ran against. It holds the files the scan loaded.
    watch_files: Arc<FxDashSet<ArcStr>>,
    /// Whether a lost HMR update is pending. Then the coordinator queues a `FullBuild`, and the
    /// task reloads every client (`BundlingTask::run_inner`).
    recover_lost_update: bool,
  },
  Close,
}
