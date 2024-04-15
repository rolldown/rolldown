mod build_error;
mod diagnostic;
mod error;
mod event_kind;
mod events;
mod inter_error;
mod utils;

use std::{borrow::Cow, path::Path};

use sugar_path::SugarPath;

// Rolldown recoverable Error.
pub use crate::error::{Error, Result};
// The `BuildError` is a rolldown diagnostic error, it will be used to report error in the build process, including at `Output#errors`.
pub use crate::inter_error::{InterError, InternalResult};
// The `InterError` is a enum to wrap the recoverable error and diagnostic error.
pub use crate::{
  build_error::{BuildError, BuildResult},
  event_kind::EventKind,
};

trait PathExt {
  fn relative_display(&self) -> Cow<str>;
}

impl PathExt for Path {
  fn relative_display(&self) -> Cow<str> {
    let cwd = std::env::current_dir().expect("Failed to get current directory");
    let non_absolute = if self.is_absolute() {
      Cow::Owned(self.relative(cwd).to_slash_lossy().into_owned())
    } else {
      self.to_string_lossy()
    };

    non_absolute
  }
}
