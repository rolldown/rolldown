use crate::dev::{dev_context::BundlingFuture, types::initial_build_state::InitialBuildState};

/// Response containing current coordinator status
#[derive(Debug, Clone)]
pub struct CoordinatorStatus {
  // `None` if no build is running
  pub running_future: Option<BundlingFuture>,
  pub has_stale_output: bool,
  pub initial_build_state: InitialBuildState,
}
