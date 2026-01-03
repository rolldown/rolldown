use rolldown_fs_watcher::FsEventResult;

use crate::type_aliases::{
  EnsureLatestBundleOutputSender, GetStateSender, GetWatchedFilesSender, ScheduleBuildIfStaleSender,
};

/// Messages sent to the BundleCoordinator
#[derive(Debug)]
pub enum CoordinatorMsg {
  WatchEvent(FsEventResult),
  BundleCompleted {
    has_encountered_error: bool,
    has_generated_bundle_output: bool,
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
  #[cfg(feature = "testing")]
  GetWatchedFiles {
    reply: GetWatchedFilesSender,
  },
  Close,
}
