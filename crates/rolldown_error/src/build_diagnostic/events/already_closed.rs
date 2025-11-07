use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

use super::BuildEvent;

#[derive(Debug)]
pub struct AlreadyClosed {}

impl BuildEvent for AlreadyClosed {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    EventKind::AlreadyClosedError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    r#"Bundle is already closed, no more calls to "generate" or "write" are allowed."#.to_string()
  }
}
