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
  },
  Close {
    reply: CloseSender,
  },
}
