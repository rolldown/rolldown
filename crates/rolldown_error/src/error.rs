use crate::BuildError;
use std::{fmt::Display, vec};

#[derive(Debug)]
pub enum Error {
  Single(BuildError),
  Vec(Vec<BuildError>),
}

impl Error {
  pub fn into_vec(self) -> Vec<BuildError> {
    match self {
      Self::Single(e) => vec![e],
      Self::Vec(es) => es,
    }
  }
}

impl Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Single(e) => e.fmt(f),
      Self::Vec(errors) => {
        if let Some(e) = errors.first() {
          e.fmt(f)?;
        }
        Ok(())
      }
    }
  }
}

impl From<std::io::Error> for Error {
  fn from(err: std::io::Error) -> Self {
    Error::Single(err.into())
  }
}

impl From<napi::Error> for Error {
  fn from(err: napi::Error) -> Self {
    Error::Single(err.into())
  }
}

impl From<BuildError> for Error {
  fn from(err: BuildError) -> Self {
    Error::Single(err)
  }
}

impl From<Vec<BuildError>> for Error {
  fn from(err: Vec<BuildError>) -> Self {
    Error::Vec(err)
  }
}

impl From<Vec<Error>> for Error {
  fn from(err: Vec<Error>) -> Self {
    let mut errors = vec![];
    for e in err {
      match e {
        Error::Single(e) => errors.push(e),
        Error::Vec(mut es) => errors.append(&mut es),
      }
    }
    Error::Vec(errors)
  }
}

pub type Result<T> = std::result::Result<T, Error>;
