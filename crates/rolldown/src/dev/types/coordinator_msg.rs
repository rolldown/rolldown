use rolldown_error::BuildResult;
use rolldown_fs_watcher::FileChangeResult;

use crate::dev::type_aliases::{GetStatusSender, ScheduleBuildSender};

/// Messages sent to the BundleCoordinator
pub enum CoordinatorMsg {
  /// File system change event from watcher
  WatchEvent(FileChangeResult),
  /// Build task completed
  BundleCompleted { result: BuildResult<()>, has_generated_bundle_output: bool },
  /// Request to schedule a build if stale
  ScheduleBuild { reply: ScheduleBuildSender },
  /// Get current build status (atomic operation)
  GetStatus { reply: GetStatusSender },
  /// Close the coordinator
  Close,
}
