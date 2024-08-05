use crate::events::BuildEvent;
use crate::{DiagnosticOptions, EventKind};

#[derive(Debug)]
pub struct MissingNameOptionForIifeExport {}

impl BuildEvent for MissingNameOptionForIifeExport {
  fn kind(&self) -> EventKind {
    EventKind::MissingNameOptionForIifeExport
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    "If you do not supply \"output.name\", you may not be able to access the exports of an IIFE bundle.".to_string()
  }
}
