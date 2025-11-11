/// State of the initial build process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialBuildState {
  /// Initial build is currently in progress
  InProgress,
  /// Initial build completed successfully
  Succeeded,
  /// Initial build failed with errors
  Failed,
}
