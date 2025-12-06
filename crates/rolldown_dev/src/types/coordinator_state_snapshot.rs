use crate::dev_context::BundlingFuture;

/// Response containing current coordinator status
#[derive(Debug, Clone)]
#[expect(clippy::struct_excessive_bools)]
pub struct CoordinatorStateSnapshot {
  // `None` if no build is running
  pub running_future: Option<BundlingFuture>,
  pub last_full_build_failed: bool,
  pub has_stale_output: bool,
  /// `true` if the coordinator is in an error state (FullBuildFailed or Failed)
  pub is_in_error_state: bool,
  /// `true` if there are queued tasks waiting to be processed
  pub has_queued_tasks: bool,
}
