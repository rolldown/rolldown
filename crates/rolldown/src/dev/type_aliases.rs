use tokio::sync::{
  mpsc::{UnboundedReceiver, UnboundedSender},
  oneshot,
};

use super::types::{
  coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot,
  ensure_latest_bundle_output_return::EnsureLatestBundleOutputReturn,
  schedule_build_return::ScheduleBuildReturn,
};

// GetBuildStatus message
pub type GetStateSender = oneshot::Sender<CoordinatorStateSnapshot>;
pub type GetStateReceiver = oneshot::Receiver<CoordinatorStateSnapshot>;

// ScheduleBuild message
pub type ScheduleBuildIfStaleSender = oneshot::Sender<Option<ScheduleBuildReturn>>;
pub type ScheduleBuildIfStaleReceiver = oneshot::Receiver<Option<ScheduleBuildReturn>>;

// Coordinator channel
pub type CoordinatorSender = UnboundedSender<CoordinatorMsg>;
pub type CoordinatorReceiver = UnboundedReceiver<CoordinatorMsg>;

pub type EnsureLatestBundleOutputSender = oneshot::Sender<Option<EnsureLatestBundleOutputReturn>>;
pub type EnsureLatestBundleOutputReceiver =
  oneshot::Receiver<Option<EnsureLatestBundleOutputReturn>>;
