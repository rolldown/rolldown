use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
use arcstr::ArcStr;

use super::BuildEvent;

#[derive(Debug)]
pub struct FilenameConflict {
  pub filename: ArcStr,
}

impl BuildEvent for FilenameConflict {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    EventKind::FilenameConflict
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "The emitted file {} overwrites a previously emitted file of the same name.",
      self.filename
    )
  }
}
