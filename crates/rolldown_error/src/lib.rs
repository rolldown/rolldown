mod error;
mod error_kind;
mod utils;

use std::path::Path;

use sugar_path::SugarPath;

// pub use crate::{diagnostic::Diagnostic, error::BuildError};
pub use crate::error::BuildError;

trait PathExt {
  fn relative_display(&self) -> String;
}

impl PathExt for Path {
  fn relative_display(&self) -> String {
    // FIXME: we should use the same cwd as the user passed to rolldown
    let cwd = std::env::current_dir().unwrap();
    if self.is_absolute() {
      self.relative(cwd).display().to_string()
    } else {
      self.display().to_string()
    }
  }
}
