use crate::types::diagnostic_options::DiagnosticOptions;

use super::BuildEvent;

#[derive(Debug)]
pub struct UnsupportedTsconfigOption {
  pub message: String,
}

impl BuildEvent for UnsupportedTsconfigOption {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::UnsupportedTsconfigOption
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    self.message.clone()
  }
}
