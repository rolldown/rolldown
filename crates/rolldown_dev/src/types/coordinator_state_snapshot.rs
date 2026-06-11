use crate::{dev_context::BundlingFuture, types::error_stage::ErrorStage};

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
  /// The stage that produced the last incremental failure, when the
  /// coordinator is in `Failed { .. }`. `None` for the success path and
  /// for `FullBuildFailed` (a full build covers every stage, so there is
  /// no single originating stage to report; use `last_build_errored` to
  /// detect that case).
  ///
  /// Consumers use this as an escape hatch: an `Hmr`-stage failure may be
  /// a bug in HMR generation, so a page reload can force a full rebuild
  /// (via `trigger_full_build`) rather than replaying the cached HMR
  /// error. See `meta/design/dev-engine.md` §12.
  pub last_error_stage: Option<ErrorStage>,
  pub has_stale_output: bool,
}
