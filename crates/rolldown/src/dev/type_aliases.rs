use rolldown_error::BuildResult;
use tokio::sync::{
  mpsc::{UnboundedReceiver, UnboundedSender},
  oneshot,
};

use super::{
  dev_context::BuildProcessFuture,
  types::{bundling_status::BundlingStatus, coordinator_msg::CoordinatorMsg},
};

// GetBuildStatus message
pub type GetBuildStatusSender = oneshot::Sender<BundlingStatus>;
pub type GetBuildStatusReceiver = oneshot::Receiver<BundlingStatus>;

// ScheduleBuild message
pub type ScheduleBuildSender = oneshot::Sender<BuildResult<Option<(BuildProcessFuture, bool)>>>;
pub type ScheduleBuildReceiver = oneshot::Receiver<BuildResult<Option<(BuildProcessFuture, bool)>>>;

// HasLatestBuildOutput message
pub type HasLatestBuildOutputSender = oneshot::Sender<bool>;
pub type HasLatestBuildOutputReceiver = oneshot::Receiver<bool>;

// EnsureCurrentBuildFinish message
pub type EnsureCurrentBuildFinishSender = oneshot::Sender<()>;
pub type EnsureCurrentBuildFinishReceiver = oneshot::Receiver<()>;

// Coordinator channel
pub type CoordinatorSender = UnboundedSender<CoordinatorMsg>;
pub type CoordinatorReceiver = UnboundedReceiver<CoordinatorMsg>;
