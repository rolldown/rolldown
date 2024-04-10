mod build_error;
mod diagnostic;
mod event_kind;
mod events;
mod utils;

use std::{borrow::Cow, path::Path};

use sugar_path::SugarPath;

pub use crate::{build_error::BuildError, event_kind::EventKind};

pub type Result<T> = std::result::Result<T, BuildError>;

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
