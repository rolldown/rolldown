use crate::{dev_context::BundlingFuture, types::error_stage::ErrorStage};
use rolldown_dev_common::types::DevCallbackError;

/// Response containing current coordinator status
#[derive(Debug, Clone)]
pub struct CoordinatorStateSnapshot {
  // `None` if no build is running
  pub running_future: Option<BundlingFuture>,
  /// True when the coordinator is in any error state — `FullBuildFailed`
  /// OR `Failed { .. }`.
  ///
  /// Consumers should use this to gate access-triggered rebuilds
  /// (`internal-docs/dev-engine/design.md`, principle 1) so a stale +
  /// errored bundle doesn't keep retriggering work that the engine will
  /// no-op anyway.
  pub last_build_errored: bool,
  /// The stage that produced the last incremental failure, when the
  /// coordinator is in `Failed { .. }`. `None` for the success path and
  /// for `FullBuildFailed` (a full build covers every stage, so there is
  /// no single originating stage to report; use `last_build_errored` to
  /// detect that case).
  ///
  /// Consumers use this as an escape hatch: an `Hmr`-stage failure may be
  /// a bug in HMR generation, so a page reload can force a full rebuild
  /// (via `trigger_full_build`) rather than replaying the cached HMR
  /// error. See `internal-docs/dev-engine/implementation.md` §12.
  pub last_error_stage: Option<ErrorStage>,
  /// Last callback execution failure. Cleared when a subsequent task starts.
  pub last_callback_error: Option<DevCallbackError>,
  pub has_stale_output: bool,
}
