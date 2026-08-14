#[cfg(feature = "testing")]
use rustc_hash::FxHashSet;
use tokio::sync::{
  mpsc::{UnboundedReceiver, UnboundedSender},
  oneshot,
};

#[cfg(feature = "testing")]
use super::types::schedule_build_return::ScheduleBuildReturn;
use super::types::{
  coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot,
  ensure_latest_bundle_output_return::EnsureLatestBundleOutputReturn,
};

// GetBuildStatus message
pub type GetStateSender = oneshot::Sender<CoordinatorStateSnapshot>;

// ScheduleBuild message
#[cfg(feature = "testing")]
pub type ScheduleBuildIfStaleSender = oneshot::Sender<Option<ScheduleBuildReturn>>;

// Coordinator channel
pub type CoordinatorSender = UnboundedSender<CoordinatorMsg>;
pub type CoordinatorReceiver = UnboundedReceiver<CoordinatorMsg>;

pub type EnsureLatestBundleOutputSender = oneshot::Sender<Option<EnsureLatestBundleOutputReturn>>;

#[cfg(feature = "testing")]
pub type GetWatchedFilesSender = oneshot::Sender<FxHashSet<String>>;
