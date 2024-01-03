use rolldown_error::BuildError;
use smallvec::SmallVec;

#[derive(Debug, Default)]
pub struct BatchedErrors(SmallVec<[BuildError; 1]>);

impl BatchedErrors {
  // TODO(hyf): using `trait Extend` would be more proper.
  pub fn merge(&mut self, mut other: Self) {
    self.0.append(&mut other.0);
  }

  pub fn with_error(err: BuildError) -> Self {
    Self(smallvec::smallvec![err])
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn push(&mut self, err: BuildError) {
    self.0.push(err);
  }

  pub fn get(&self) -> Option<&BuildError> {
    self.0.first()
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

pub fn collect_errors<T>(value: Vec<Result<T, BuildError>>) -> BatchedResult<Vec<T>> {
  let mut errors = BatchedErrors::default();

  let collected = value.into_iter().filter_map(|item| errors.take_err_from(item)).collect();

  if errors.is_empty() {
    Ok(collected)
  } else {
    Err(errors)
  }
}

pub type BatchedResult<T> = Result<T, BatchedErrors>;

impl From<BuildError> for BatchedErrors {
  fn from(err: BuildError) -> Self {
    Self::with_error(err)
  }
}

impl From<std::io::Error> for BatchedErrors {
  fn from(err: std::io::Error) -> Self {
    Self::with_error(err.into())
  }
}

impl From<BatchedErrors> for Vec<BuildError> {
  fn from(errs: BatchedErrors) -> Self {
    errs.0.into_vec()
  }
}
