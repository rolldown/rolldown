use rolldown_error::BuildError;
use smallvec::SmallVec;

#[derive(Debug, Default)]
pub struct BatchedErrors(SmallVec<[BuildError; 1]>);

impl BatchedErrors {
  pub fn with_error(err: BuildError) -> Self {
    Self(smallvec::smallvec![err])
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn push(&mut self, err: BuildError) {
    self.0.push(err);
  }

  /// Try to take the Err() of the given result and return Some(T) if it's Ok(T).
  pub fn take_err_from<T>(&mut self, res: Result<T, rolldown_error::BuildError>) -> Option<T> {
    match res {
      Ok(t) => Some(t),
      Err(err) => {
        self.push(err);
        None
      }
    }
  }
}

pub type BatchedResult<T> = Result<T, BatchedErrors>;

impl From<BuildError> for BatchedErrors {
  fn from(err: BuildError) -> Self {
    Self::with_error(err)
  }
}

impl From<BatchedErrors> for Vec<BuildError> {
  fn from(errs: BatchedErrors) -> Self {
    errs.0.into_vec()
  }
}
