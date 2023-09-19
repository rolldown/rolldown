use std::path::PathBuf;

use crate::Error;

/// A collection of rolldown [Error].
///
/// Yeah, this is just a wrapper of `Vec<Error>` but with a few helpful methods:
#[allow(unused)]
#[derive(Debug, Default)]
pub struct Errors {
  errors: Vec<Error>,
  cwd: Option<PathBuf>,
}

impl Errors {
  pub fn new(err: Error) -> Self {
    Self {
      errors: vec![err],
      cwd: None,
    }
  }

  pub fn push(&mut self, error: Error) {
    self.errors.push(error);
  }

  pub fn into_vec(self) -> Vec<Error> {
    self.errors
  }
}

impl Extend<Error> for Errors {
  fn extend<T: IntoIterator<Item = Error>>(&mut self, iter: T) {
    self.errors.extend(iter)
  }
}
