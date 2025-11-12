use rolldown_error::BuildResult;
use rolldown_fs_watcher::FileChangeResult;

use crate::dev::type_aliases::{GetStatusSender, HasLatestBuildOutputSender, ScheduleBuildSender};

/// Messages sent to the BundleCoordinator
pub enum CoordinatorMsg {
  /// File system change event from watcher
  WatchEvent(FileChangeResult),
  /// Build task completed
  BuildCompleted { result: BuildResult<()>, task_required_rebuild: bool },
  /// Request to schedule a build if stale
  ScheduleBuild { reply: ScheduleBuildSender },
  /// Check if we have latest build output
  HasLatestBuildOutput { reply: HasLatestBuildOutputSender },
  /// Get current build status (atomic operation)
  GetStatus { reply: GetStatusSender },
  /// Close the coordinator
  Close,
}
