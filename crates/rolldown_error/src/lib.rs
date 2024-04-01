mod diagnostic;
mod error;
mod error_kind;
mod utils;

use std::{borrow::Cow, path::Path};

use sugar_path::SugarPath;

pub(crate) type StaticStr = Cow<'static, str>;

// pub use crate::{diagnostic::Diagnostic, error::BuildError};
pub use crate::error::BuildError;

trait PathExt {
  fn relative_display(&self) -> String;
}

impl PathExt for Path {
  fn relative_display(&self) -> String {
    // TODO: Should have a global cache for `cwd` using once_cell
    let cwd = std::env::current_dir().expect("Failed to get current directory");
    let ret = if self.is_absolute() {
      self.relative(cwd).display().to_string()
    } else {
      self.display().to_string()
    };
    // TODO: should move this to `sugar_path`
    ret.replace('\\', "/")
  }
}
