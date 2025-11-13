/// State of the initial build process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorState {
  Initialized,
  Idle,
  FullBuildInProgress,
  FullBuildFailed,
  InProgress,
  Failed,
}
