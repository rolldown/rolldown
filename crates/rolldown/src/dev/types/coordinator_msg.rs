use rolldown_error::BuildResult;
use rolldown_fs_watcher::FsEventResult;

use crate::dev::type_aliases::{
  EnsureLatestBundleOutputSender, GetStatusSender, ScheduleBuildIfStaleSender,
};

/// Messages sent to the BundleCoordinator
#[derive(Debug)]
pub enum CoordinatorMsg {
  WatchEvent(FsEventResult),
  BundleCompleted { result: BuildResult<()>, has_generated_bundle_output: bool },
  ScheduleBuildIfStale { reply: ScheduleBuildIfStaleSender },
  GetStatus { reply: GetStatusSender },
  EnsureLatestBundleOutput { reply: EnsureLatestBundleOutputSender },
  Close,
}
