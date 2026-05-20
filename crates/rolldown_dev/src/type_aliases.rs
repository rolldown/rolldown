use futures::channel::oneshot;
use rustc_hash::FxHashSet;

use super::types::{
  coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot,
  ensure_latest_bundle_output_return::EnsureLatestBundleOutputReturn,
  schedule_build_return::ScheduleBuildReturn,
};

// GetBuildStatus message
pub type GetStateSender = oneshot::Sender<CoordinatorStateSnapshot>;

// ScheduleBuild message
pub type ScheduleBuildIfStaleSender = oneshot::Sender<Option<ScheduleBuildReturn>>;

// Coordinator channel
pub type CoordinatorSender = async_channel::Sender<CoordinatorMsg>;
pub type CoordinatorReceiver = async_channel::Receiver<CoordinatorMsg>;

pub type EnsureLatestBundleOutputSender = oneshot::Sender<Option<EnsureLatestBundleOutputReturn>>;

pub type GetWatchedFilesSender = oneshot::Sender<FxHashSet<String>>;
