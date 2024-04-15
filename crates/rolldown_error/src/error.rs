use crate::BuildError;

// The `BuildError` is a rolldown diagnostic error, it will be used to report error in the build process, including at `Output#errors`.

// Rolldown recoverable Error.
pub type Error = anyhow::Error;
pub type Result<T> = anyhow::Result<T>;

// The `InterError` is a enum to wrap the recoverable error and diagnostic error.
pub enum InterError {
  Err(anyhow::Error),
  BuildError(BuildError),
}
pub type InternalResult<T> = std::result::Result<T, InterError>;

impl From<anyhow::Error> for InterError {
  fn from(err: anyhow::Error) -> Self {
    Self::Err(err)
  }
}

impl From<BuildError> for InterError {
  fn from(err: BuildError) -> Self {
    Self::BuildError(err)
  }
}
