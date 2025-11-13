use crate::dev::dev_context::BundlingFuture;

/// Response containing current coordinator status
#[derive(Debug, Clone)]
pub struct CoordinatorStateSnapshot {
  // `None` if no build is running
  pub running_future: Option<BundlingFuture>,
  pub has_stale_output: bool,
}
