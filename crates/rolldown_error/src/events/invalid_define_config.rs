use crate::{event_kind::EventKind, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct InvalidDefineConfig {
  pub message: String,
}

impl BuildEvent for InvalidDefineConfig {
  fn kind(&self) -> crate::event_kind::EventKind {
    EventKind::InvalidDefineConfigError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    self.message.clone()
  }
}
