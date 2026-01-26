use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct DuplicateShebang {
  pub filename: String,
}

impl BuildEvent for DuplicateShebang {
  fn kind(&self) -> EventKind {
    EventKind::DuplicateShebang
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Both the code and postBanner contain shebang in \"{}\". This will cause a syntax error.",
      self.filename
    )
  }
}
