use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
use arcstr::ArcStr;
use rolldown_utils::concat_string;

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
    concat_string!(
      "The emitted file ",
      self.filename,
      " overwrites a previously emitted file of the same name."
    )
  }
}
