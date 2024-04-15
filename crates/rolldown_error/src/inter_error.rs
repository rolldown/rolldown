use crate::BuildError;

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
