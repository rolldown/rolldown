use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
use arcstr::ArcStr;

use super::BuildEvent;

#[derive(Debug)]
pub enum FilenameConflict {
  File(ArcStr),
  FileAndDirectory(ArcStr),
}

impl BuildEvent for FilenameConflict {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    EventKind::FilenameConflict
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    match self {
      Self::File(filename) => {
        format!(
          "The emitted file {filename} overwrites a previously emitted file of the same name."
        )
      }
      Self::FileAndDirectory(path) => {
        format!("The emitted files use {path} as a file and as a directory.")
      }
    }
  }
}
