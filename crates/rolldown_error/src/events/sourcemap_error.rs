use crate::types::diagnostic_options::DiagnosticOptions;

use super::BuildEvent;

#[derive(Debug)]
pub struct SourceMapError {
  pub error: oxc::sourcemap::Error,
}

impl BuildEvent for SourceMapError {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::SourcemapError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Error when using sourcemap for reporting an error: {:?}", self.error)
  }
}
