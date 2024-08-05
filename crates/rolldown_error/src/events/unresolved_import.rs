use crate::types::diagnostic_options::DiagnosticOptions;

use super::BuildEvent;

#[derive(Debug)]
pub struct UnresolvedImport {
  pub(crate) resolved: String,
  pub(crate) importer: Option<String>,
  pub(crate) reason: String,
}

impl BuildEvent for UnresolvedImport {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::UnresolvedImport
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Could not resolve {}{} - {}.",
      self.resolved,
      self.importer.as_ref().map(|i| format!(" (imported by {})", i)).unwrap_or_default(),
      self.reason
    )
  }
}
