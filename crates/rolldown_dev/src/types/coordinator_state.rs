use crate::types::error_stage::ErrorStage;

/// State of the initial build process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorState {
  Initialized,
  Idle,
  FullBuildInProgress,
  FullBuildFailed,
  InProgress,
  /// Incremental task errored. The carried stage drives the recovery
  /// choice in `handle_file_changes` — see
  /// `internal-docs/dev-engine/implementation.md` §7 and the principles in
  /// `internal-docs/dev-engine/design.md`.
  Failed {
    last_error_stage: ErrorStage,
  },
}
