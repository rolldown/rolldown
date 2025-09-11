use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

use super::BuildEvent;

#[derive(Debug)]
pub struct InvalidDefineConfig {
  pub message: String,
}

impl BuildEvent for InvalidDefineConfig {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    EventKind::InvalidDefineConfigError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    self.message.clone()
  }
}
