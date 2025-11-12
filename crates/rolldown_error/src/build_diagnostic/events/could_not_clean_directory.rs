use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct CouldNotCleanDirectory {
  pub dir: String,
  pub reason: String,
}

impl BuildEvent for CouldNotCleanDirectory {
  fn kind(&self) -> EventKind {
    EventKind::CouldNotCleanDirectory
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Could not clean directory for output chunks: {}. Reason: {}", self.dir, self.reason)
  }
}
