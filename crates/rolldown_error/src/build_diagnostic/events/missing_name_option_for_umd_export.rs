use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct MissingNameOptionForUmdExport {}

impl BuildEvent for MissingNameOptionForUmdExport {
  fn kind(&self) -> EventKind {
    EventKind::MissingNameOptionForUmdExportError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    "You must supply `output.name` for UMD bundles that have exports so that the exports are accessible in environments without a module loader.".to_string()
  }
}
