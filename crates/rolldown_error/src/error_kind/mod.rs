use std::fmt::Debug;

pub mod external_entry;
pub mod unresolved_entry;
pub mod unresolved_import;

// TODO(hyf0): Not a good name, probably should rename to `BuildError`
pub trait BuildErrorLike: Debug + Sync + Send {
  fn code(&self) -> &'static str;

  fn message(&self) -> String;
}

impl<T: BuildErrorLike + 'static> From<T> for Box<dyn BuildErrorLike> {
  fn from(e: T) -> Self {
    Box::new(e)
  }
}

// --- TODO(hyf0): These errors are only for compatibility with legacy code. They should be replaced with more specific errors.

#[derive(Debug)]
pub struct NapiError {
  pub status: String,
  pub reason: String,
}

impl BuildErrorLike for NapiError {
  fn code(&self) -> &'static str {
    "NAPI_ERROR"
  }

  fn message(&self) -> String {
    format!("Napi error: {status}: {reason}", status = self.status, reason = self.reason)
  }
}

impl BuildErrorLike for std::io::Error {
  fn code(&self) -> &'static str {
    "IO_ERROR"
  }

  fn message(&self) -> String {
    format!("IO error: {self}")
  }
}

// --- end
