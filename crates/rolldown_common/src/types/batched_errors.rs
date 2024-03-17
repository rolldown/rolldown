use rolldown_error::BuildError;

#[derive(Debug, Default)]
pub struct BatchedErrors(Vec<BuildError>);

impl BatchedErrors {
  pub fn with_error(err: BuildError) -> Self {
    Self(vec![err])
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
pub trait IntoBatchedResult<T> {
  /// Convert an iterator of results into a single result, collecting all errors into a BatchedErrors.
  fn into_batched_result(self) -> BatchedResult<Vec<T>>;
}

impl<T, U> IntoBatchedResult<U> for T
where
  T: IntoIterator<Item = Result<U, BuildError>>,
{
  fn into_batched_result(self) -> BatchedResult<Vec<U>> {
    let mut errors = BatchedErrors::default();
    let collected =
      self.into_iter().filter_map(|item| errors.take_err_from(item)).collect::<Vec<_>>();

    if errors.is_empty() {
      Ok(collected)
    } else {
      Err(errors)
    }
  }
}

impl Extend<BuildError> for BatchedErrors {
  fn extend<T: IntoIterator<Item = BuildError>>(&mut self, iter: T) {
    self.0.extend(iter.into_iter());
  }
}

impl IntoIterator for BatchedErrors {
  type Item = BuildError;
  type IntoIter = std::vec::IntoIter<BuildError>;

  fn into_iter(self) -> Self::IntoIter {
    self.0.into_iter()
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
    errs.0
  }
}
