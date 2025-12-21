use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct MissingNameOptionForIifeExport {
  pub is_umd: bool,
}

impl BuildEvent for MissingNameOptionForIifeExport {
  fn kind(&self) -> EventKind {
    EventKind::MissingNameOptionForIifeExport
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    if self.is_umd {
      "You must supply `output.name` for UMD bundles that have exports so that the exports are accessible in environments without a module loader.".to_string()
    } else {
      "If you do not supply \"output.name\", you may not be able to access the exports of an IIFE bundle.".to_string()
    }
  }
}
