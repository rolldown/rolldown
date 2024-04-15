mod build_error;
mod diagnostic;
mod error;
mod event_kind;
mod events;
mod utils;

use std::{borrow::Cow, path::Path};

use sugar_path::SugarPath;

pub use crate::error::{Error, InterError, InternalResult, Result};
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
