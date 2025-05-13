use crate::types::diagnostic_options::DiagnosticOptions;

use super::BuildEvent;

#[derive(Debug)]
pub struct UnresolvedImport {
  pub(crate) specifier: String,
  pub(crate) importer: String,
}

impl BuildEvent for UnresolvedImport {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::UnresolvedImport
  }

  fn id(&self) -> Option<String> {
    Some(self.importer.clone())
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!("Could not resolve {} from {}.", self.specifier, opts.stabilize_path(&self.importer))
  }
}
