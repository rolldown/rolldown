use rolldown_error::BuildResult;
use tokio::sync::{
  mpsc::{UnboundedReceiver, UnboundedSender},
  oneshot,
};

use super::{
  dev_context::BundlingFuture,
  types::{
    coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot,
    schedule_build_return::ScheduleBuildReturn,
  },
};

// GetBuildStatus message
pub type GetStateSender = oneshot::Sender<CoordinatorStateSnapshot>;
pub type GetStateReceiver = oneshot::Receiver<CoordinatorStateSnapshot>;

// ScheduleBuild message
pub type ScheduleBuildIfStaleSender = oneshot::Sender<BuildResult<Option<ScheduleBuildReturn>>>;
pub type ScheduleBuildIfStaleReceiver = oneshot::Receiver<BuildResult<Option<ScheduleBuildReturn>>>;

// Coordinator channel
pub type CoordinatorSender = UnboundedSender<CoordinatorMsg>;
pub type CoordinatorReceiver = UnboundedReceiver<CoordinatorMsg>;

pub type EnsureLatestBundleOutputSender = oneshot::Sender<BuildResult<Option<BundlingFuture>>>;
pub type EnsureLatestBundleOutputReceiver = oneshot::Receiver<BuildResult<Option<BundlingFuture>>>;
