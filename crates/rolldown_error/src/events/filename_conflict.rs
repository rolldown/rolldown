use crate::{event_kind::EventKind, types::diagnostic_options::DiagnosticOptions};
use oxc::span::CompactStr;
use rolldown_utils::concat_string;

use super::BuildEvent;

#[derive(Debug)]
pub struct FilenameConflict {
  pub filename: CompactStr,
}

impl BuildEvent for FilenameConflict {
  fn kind(&self) -> crate::event_kind::EventKind {
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
