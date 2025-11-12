use rolldown_error::BuildResult;
use tokio::sync::{
  mpsc::{UnboundedReceiver, UnboundedSender},
  oneshot,
};

use super::{
  dev_context::BundlingFuture,
  types::{coordinator_msg::CoordinatorMsg, coordinator_status::CoordinatorStatus},
};

// GetBuildStatus message
pub type GetStatusSender = oneshot::Sender<CoordinatorStatus>;
pub type GetStatusReceiver = oneshot::Receiver<CoordinatorStatus>;

// ScheduleBuild message
pub type ScheduleBuildSender = oneshot::Sender<BuildResult<Option<(BundlingFuture, bool)>>>;
pub type ScheduleBuildReceiver = oneshot::Receiver<BuildResult<Option<(BundlingFuture, bool)>>>;

// HasLatestBuildOutput message
pub type HasLatestBuildOutputSender = oneshot::Sender<bool>;
pub type HasLatestBuildOutputReceiver = oneshot::Receiver<bool>;

// Coordinator channel
pub type CoordinatorSender = UnboundedSender<CoordinatorMsg>;
pub type CoordinatorReceiver = UnboundedReceiver<CoordinatorMsg>;
