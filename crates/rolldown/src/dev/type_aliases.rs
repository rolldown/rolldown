use rolldown_error::BuildResult;
use tokio::sync::{
  mpsc::{UnboundedReceiver, UnboundedSender},
  oneshot,
};

use super::{
  dev_context::BundlingFuture,
  types::{
    coordinator_msg::CoordinatorMsg, coordinator_status::CoordinatorStatus,
    schedule_build_return::ScheduleBuildReturn,
  },
};

// GetBuildStatus message
pub type GetStatusSender = oneshot::Sender<CoordinatorStatus>;
pub type GetStatusReceiver = oneshot::Receiver<CoordinatorStatus>;

// ScheduleBuild message
pub type ScheduleBuildIfStaleSender = oneshot::Sender<BuildResult<Option<ScheduleBuildReturn>>>;
pub type ScheduleBuildIfStaleReceiver = oneshot::Receiver<BuildResult<Option<ScheduleBuildReturn>>>;

// Coordinator channel
pub type CoordinatorSender = UnboundedSender<CoordinatorMsg>;
pub type CoordinatorReceiver = UnboundedReceiver<CoordinatorMsg>;

pub type EnsureLatestBundleOutputSender = oneshot::Sender<BuildResult<Option<BundlingFuture>>>;
pub type EnsureLatestBundleOutputReceiver = oneshot::Receiver<BuildResult<Option<BundlingFuture>>>;
