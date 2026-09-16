use arcstr::ArcStr;
use rolldown_dev_common::types::DevCallbackError;
use rolldown_fs_watcher::FsEventResult;

use crate::type_aliases::{
  BeginWatchRegistrationErrorObservationSender, CloseSender, EnsureLatestBundleOutputSender,
  GetStateSender, PreviewWatchRegistrationErrorsSender, WatchRegistrationErrorObserverId,
};
#[cfg(feature = "testing")]
use crate::type_aliases::{GetWatchedFilesSender, ScheduleBuildIfStaleSender};
use crate::types::error_stage::ErrorStage;

/// Messages sent to the BundleCoordinator
#[derive(Debug)]
pub enum CoordinatorMsg {
  WatchEvent(FsEventResult),
  BundleCompleted {
    /// `None` on success; on error, identifies which stage produced it
    /// so the coordinator can pick the right recovery task variant on
    /// the next file change. See `internal-docs/dev-engine/implementation.md` §7.
    error_stage: Option<ErrorStage>,
    has_generated_bundle_output: bool,
    /// Callback execution failure, retained separately from build diagnostics
    /// so lifecycle waiters can observe rejected/throwing consumer callbacks.
    callback_error: Option<DevCallbackError>,
    /// `plugin_driver.watch_files` as it stood right before this task's rebuild
    /// replaced the bundle handle — empty when the task never rebuilt.
    ///
    /// The HMR stage runs on the handle the rebuild is about to retire, and a
    /// module it pulls in (a new import) is merged into the scan cache, so the
    /// rebuild's partial scan no longer refetches it and the fresh handle never
    /// records it. The coordinator reads the handle after the lock is gone, so
    /// re-reading it observes only what the rebuild rescanned; the rest would go
    /// unwatched forever. Same shape as `ModuleChanged::watch_files`.
    watch_files: Vec<ArcStr>,
  },
  #[cfg(feature = "testing")]
  ScheduleBuildIfStale {
    reply: ScheduleBuildIfStaleSender,
  },
  GetState {
    reply: GetStateSender,
  },
  BeginWatchRegistrationErrorObservation {
    reply: BeginWatchRegistrationErrorObservationSender,
  },
  PreviewWatchRegistrationErrors {
    observer_id: WatchRegistrationErrorObserverId,
    reply: PreviewWatchRegistrationErrorsSender,
  },
  AcknowledgeWatchRegistrationErrors {
    observer_id: WatchRegistrationErrorObserverId,
  },
  CancelWatchRegistrationErrorObservation {
    observer_id: WatchRegistrationErrorObserverId,
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
    /// `plugin_driver.watch_files` as it stood when the sender still held the
    /// bundler lock, so the paths this change added cannot be lost.
    ///
    /// Those entries live on the current bundle handle only, and a rebuild
    /// installs a fresh handle with an empty set. The coordinator reads the
    /// handle asynchronously, long after the sender released the lock, so
    /// re-reading it can observe a handle that never held these paths. They
    /// ride along with the message instead, and the coordinator unions them
    /// into whatever the live handle holds. See
    /// `internal-docs/dev-engine/implementation.md`.
    watch_files: Vec<ArcStr>,
  },
  Close {
    reply: CloseSender,
  },
}
