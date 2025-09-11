use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
use std::path::PathBuf;

use super::BuildEvent;

#[derive(Debug)]
pub struct ExternalEntry {
  pub(crate) id: PathBuf,
}

impl BuildEvent for ExternalEntry {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    EventKind::UnresolvedEntry
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!("Entry module {:?} cannot be external.", opts.stabilize_path(&self.id))
  }
}
