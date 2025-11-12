use crate::dev::{dev_context::BundlingFuture, types::initial_build_state::InitialBuildState};

/// Response containing current coordinator status
#[derive(Debug, Clone)]
pub struct CoordinatorStatus {
  /// The current build future if a build is running
  pub current_build_future: Option<BundlingFuture>,
  /// Whether the build output is stale
  pub has_stale_output: bool,
  /// The state of the initial build
  pub initial_build_state: InitialBuildState,
}
