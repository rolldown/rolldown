use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

use super::BuildEvent;

#[derive(Debug)]
pub struct FilenameOutsideOutputDirectory {
  pub filename: String,
}

impl BuildEvent for FilenameOutsideOutputDirectory {
  fn kind(&self) -> EventKind {
    EventKind::FilenameOutsideOutputDirectoryError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "The output file name \"{}\" is not contained in the output directory. Make sure all file names are relative paths without \"..\" segments.",
      self.filename
    )
  }
}
