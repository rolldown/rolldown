use super::BuildEvent;
use crate::types::diagnostic_options::DiagnosticOptions;
use std::path::PathBuf;

#[derive(Debug)]
pub struct UnresolvedEntry {
  pub(crate) unresolved_id: PathBuf,
}

impl BuildEvent for UnresolvedEntry {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::UnresolvedEntry
  }
  fn code(&self) -> &'static str {
    "UNRESOLVED_ENTRY"
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!("Cannot resolve entry module {}.", opts.stabilize_path(&self.unresolved_id))
  }
}
