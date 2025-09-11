use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct BundlerInitializeError {
  pub message: String,
  pub hint: Option<String>,
}

impl BuildEvent for BundlerInitializeError {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::BundlerInitializeError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    self.message.clone()
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    diagnostic.title = self.message(opts);

    if let Some(hint) = &self.hint {
      diagnostic.add_help(hint.clone());
    }
  }
}
