use crate::dev_context::BundlingFuture;

/// Response containing current coordinator status
#[derive(Debug, Clone)]
pub struct CoordinatorStateSnapshot {
  // `None` if no build is running
  pub running_future: Option<BundlingFuture>,
  /// True when the coordinator is in any error state — `FullBuildFailed`
  /// OR `Failed { .. }`.
  ///
  /// Consumers should use this to gate
  /// access-triggered rebuilds (Design principles §1) so a stale + errored bundle doesn't keep retriggering work
  /// that the engine will no-op anyway. See `meta/design/dev-engine.md`.
  pub last_build_errored: bool,
  pub has_stale_output: bool,
}
